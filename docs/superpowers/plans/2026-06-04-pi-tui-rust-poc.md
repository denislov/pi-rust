# pi-tui Rust Core Renderer PoC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Rust `pi-tui` PoC: a width-aware component renderer with terminal abstraction, virtual terminal tests, and basic differential rendering.

**Architecture:** The crate mirrors the TS `pi-tui` core philosophy: components render strings, `Tui<T: Terminal>` owns terminal output, and render frames are validated before terminal writes. The PoC uses `crossterm` only as the real terminal backend; it does not use `ratatui`.

**Tech Stack:** Rust 1.96.0, edition 2024, `crossterm`, `unicode-width`, `unicode-segmentation`, `thiserror`.

---

## File Structure

- Modify `crates/pi-tui/Cargo.toml`: add runtime dependencies.
- Replace `crates/pi-tui/src/lib.rs`: public module declarations and re-exports.
- Create `crates/pi-tui/src/utils/ansi.rs`: parse supported ANSI/OSC/APC escape sequences.
- Create `crates/pi-tui/src/utils/width.rs`: `visible_width()` and `truncate_to_width()`.
- Create `crates/pi-tui/src/utils/mod.rs`: utility module exports.
- Create `crates/pi-tui/src/component.rs`: `Component` trait and `Container`.
- Create `crates/pi-tui/src/components/text.rs`: `Text` component.
- Create `crates/pi-tui/src/components/spacer.rs`: `Spacer` component.
- Create `crates/pi-tui/src/components/mod.rs`: component module exports.
- Create `crates/pi-tui/src/terminal.rs`: `TerminalSize`, `Terminal` trait, `ProcessTerminal`.
- Create `crates/pi-tui/src/virtual_terminal.rs`: in-memory terminal backend for tests.
- Create `crates/pi-tui/src/tui.rs`: `Tui`, `RenderOutcome`, `RenderStrategy`, `TuiError`.
- Create `crates/pi-tui/tests/width.rs`: width and truncation behavior.
- Create `crates/pi-tui/tests/components.rs`: component behavior.
- Create `crates/pi-tui/tests/tui_render.rs`: render strategy behavior.
- Create `crates/pi-tui/examples/render_once.rs`: static one-shot example.

## Task 1: Configure crate dependencies and public skeleton

**Files:**
- Modify: `crates/pi-tui/Cargo.toml`
- Replace: `crates/pi-tui/src/lib.rs`

- [ ] **Step 1: Write a failing compile-facing test through public imports**

Create `crates/pi-tui/tests/public_api.rs`:

```rust
use pi_tui::{
    visible_width, Component, Container, ProcessTerminal, Spacer, Terminal, TerminalSize, Text,
    Tui, VirtualTerminal,
};

#[test]
fn public_api_symbols_are_importable() {
    assert_eq!(visible_width("abc"), 3);

    let mut container = Container::new();
    container.add_child(Box::new(Text::new("hello")));
    container.add_child(Box::new(Spacer::new(1)));
    let lines = container.render(20);
    assert_eq!(lines, vec!["hello".to_string(), "".to_string()]);

    let terminal = VirtualTerminal::new(20, 5);
    let tui = Tui::new(terminal);
    assert_eq!(tui.terminal().size(), TerminalSize { columns: 20, rows: 5 });

    let _ = std::mem::size_of::<ProcessTerminal>();
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-tui --test public_api`

Expected: FAIL to compile with unresolved imports such as `visible_width`, `Component`, and `Tui`.

- [ ] **Step 3: Add dependencies**

Update `crates/pi-tui/Cargo.toml`:

```toml
[package]
name = "pi-tui"
version = "0.1.0"
edition = "2024"

[dependencies]
crossterm = "0.28"
thiserror = "2"
unicode-segmentation = "1"
unicode-width = "0.2"
```

- [ ] **Step 4: Replace `lib.rs` with module skeleton**

Replace `crates/pi-tui/src/lib.rs`:

```rust
pub mod component;
pub mod components;
pub mod terminal;
pub mod tui;
pub mod utils;
pub mod virtual_terminal;

pub use component::{Component, Container};
pub use components::{Spacer, Text};
pub use terminal::{ProcessTerminal, Terminal, TerminalSize};
pub use tui::{RenderOutcome, RenderStrategy, Tui, TuiError};
pub use utils::{truncate_to_width, visible_width};
pub use virtual_terminal::{TerminalOp, VirtualTerminal};
```

- [ ] **Step 5: Add temporary minimal modules to satisfy paths**

Create `crates/pi-tui/src/component.rs`:

```rust
pub trait Component {
    fn render(&mut self, width: usize) -> Vec<String>;

    fn invalidate(&mut self) {}
}

pub struct Container {
    children: Vec<Box<dyn Component>>,
}

impl Container {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    pub fn add_child(&mut self, child: Box<dyn Component>) {
        self.children.push(child);
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Container {
    fn render(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for child in &mut self.children {
            lines.extend(child.render(width));
        }
        lines
    }

    fn invalidate(&mut self) {
        for child in &mut self.children {
            child.invalidate();
        }
    }
}
```

Create `crates/pi-tui/src/components/mod.rs`:

```rust
mod spacer;
mod text;

pub use spacer::Spacer;
pub use text::Text;
```

Create `crates/pi-tui/src/components/text.rs`:

```rust
use crate::Component;

pub struct Text {
    text: String,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl Component for Text {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![self.text.clone()]
    }
}
```

Create `crates/pi-tui/src/components/spacer.rs`:

```rust
use crate::Component;

pub struct Spacer {
    height: usize,
}

impl Spacer {
    pub fn new(height: usize) -> Self {
        Self { height }
    }
}

impl Component for Spacer {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![String::new(); self.height]
    }
}
```

Create `crates/pi-tui/src/terminal.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: usize,
    pub rows: usize,
}

pub trait Terminal {
    fn size(&self) -> TerminalSize;
    fn write(&mut self, data: &str) -> std::io::Result<()>;
    fn move_by(&mut self, rows: i16) -> std::io::Result<()>;
    fn hide_cursor(&mut self) -> std::io::Result<()>;
    fn show_cursor(&mut self) -> std::io::Result<()>;
    fn clear_line(&mut self) -> std::io::Result<()>;
    fn clear_from_cursor(&mut self) -> std::io::Result<()>;
    fn clear_screen(&mut self) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
}

pub struct ProcessTerminal;
```

Create `crates/pi-tui/src/virtual_terminal.rs`:

```rust
use crate::{Terminal, TerminalSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalOp {
    Write(String),
    MoveBy(i16),
    HideCursor,
    ShowCursor,
    ClearLine,
    ClearFromCursor,
    ClearScreen,
    Flush,
}

pub struct VirtualTerminal {
    size: TerminalSize,
    ops: Vec<TerminalOp>,
}

impl VirtualTerminal {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            size: TerminalSize { columns, rows },
            ops: Vec::new(),
        }
    }

    pub fn ops(&self) -> &[TerminalOp] {
        &self.ops
    }
}

impl Terminal for VirtualTerminal {
    fn size(&self) -> TerminalSize {
        self.size
    }

    fn write(&mut self, data: &str) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Write(data.to_string()));
        Ok(())
    }

    fn move_by(&mut self, rows: i16) -> std::io::Result<()> {
        self.ops.push(TerminalOp::MoveBy(rows));
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::HideCursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ShowCursor);
        Ok(())
    }

    fn clear_line(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearLine);
        Ok(())
    }

    fn clear_from_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearFromCursor);
        Ok(())
    }

    fn clear_screen(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearScreen);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Flush);
        Ok(())
    }
}
```

Create `crates/pi-tui/src/utils/mod.rs`:

```rust
mod ansi;
mod width;

pub use width::{truncate_to_width, visible_width};
```

Create `crates/pi-tui/src/utils/ansi.rs`:

```rust
pub fn ansi_sequence_len(_text: &str, _byte_pos: usize) -> Option<usize> {
    None
}
```

Create `crates/pi-tui/src/utils/width.rs`:

```rust
pub fn visible_width(text: &str) -> usize {
    text.chars().count()
}

pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    text.chars().take(max_width).collect()
}
```

Create `crates/pi-tui/src/tui.rs`:

```rust
use crate::Terminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStrategy {
    FullRedraw,
    Differential { first_changed_line: usize },
    NoChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOutcome {
    pub strategy: RenderStrategy,
    pub line_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line_index} is {width} columns wide, exceeding max width {max_width}: {line:?}")]
    LineTooWide {
        line_index: usize,
        width: usize,
        max_width: usize,
        line: String,
    },
}

pub struct Tui<T: Terminal> {
    terminal: T,
}

impl<T: Terminal> Tui<T> {
    pub fn new(terminal: T) -> Self {
        Self { terminal }
    }

    pub fn terminal(&self) -> &T {
        &self.terminal
    }
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p pi-tui --test public_api`

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/pi-tui/Cargo.toml crates/pi-tui/src crates/pi-tui/tests/public_api.rs
git commit -m "feat(tui): add core public skeleton"
```

## Task 2: Implement ANSI-aware width utilities

**Files:**
- Modify: `crates/pi-tui/src/utils/ansi.rs`
- Modify: `crates/pi-tui/src/utils/width.rs`
- Test: `crates/pi-tui/tests/width.rs`

- [ ] **Step 1: Write failing width tests**

Create `crates/pi-tui/tests/width.rs`:

```rust
use pi_tui::{truncate_to_width, visible_width};

#[test]
fn visible_width_counts_ascii() {
    assert_eq!(visible_width("hello"), 5);
}

#[test]
fn visible_width_counts_tabs_as_three_columns() {
    assert_eq!(visible_width("a\tb"), 5);
}

#[test]
fn visible_width_counts_cjk_as_wide() {
    assert_eq!(visible_width("你a"), 3);
}

#[test]
fn visible_width_counts_emoji_as_wide() {
    assert_eq!(visible_width("🙂a"), 3);
}

#[test]
fn visible_width_ignores_csi_osc_and_apc_sequences() {
    let styled = "\x1b[31mred\x1b[0m";
    let hyperlink = "\x1b]8;;https://example.com\x07link\x1b]8;;\x07";
    let marker = "\x1b_pi:c\x07x";

    assert_eq!(visible_width(styled), 3);
    assert_eq!(visible_width(hyperlink), 4);
    assert_eq!(visible_width(marker), 1);
}

#[test]
fn truncate_to_width_does_not_split_wide_graphemes() {
    assert_eq!(truncate_to_width("你好吗", 4), "你好");
    assert_eq!(truncate_to_width("🙂🙂a", 2), "🙂");
}

#[test]
fn truncate_to_width_keeps_leading_ansi_sequences() {
    let clipped = truncate_to_width("\x1b[31mhello", 2);
    assert_eq!(clipped, "\x1b[31mhe");
    assert_eq!(visible_width(&clipped), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-tui --test width`

Expected: FAIL. `visible_width_counts_cjk_as_wide`, `visible_width_counts_emoji_as_wide`, and ANSI tests should fail against the skeleton implementation.

- [ ] **Step 3: Implement ANSI sequence parsing**

Replace `crates/pi-tui/src/utils/ansi.rs`:

```rust
pub fn ansi_sequence_len(text: &str, byte_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if byte_pos >= bytes.len() || bytes[byte_pos] != 0x1b {
        return None;
    }

    let next = *bytes.get(byte_pos + 1)?;
    match next {
        b'[' => csi_len(bytes, byte_pos),
        b']' => string_sequence_len(bytes, byte_pos),
        b'_' => string_sequence_len(bytes, byte_pos),
        _ => None,
    }
}

fn csi_len(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start + 2;
    while pos < bytes.len() {
        let b = bytes[pos];
        if (0x40..=0x7e).contains(&b) {
            return Some(pos + 1 - start);
        }
        pos += 1;
    }
    None
}

fn string_sequence_len(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start + 2;
    while pos < bytes.len() {
        if bytes[pos] == 0x07 {
            return Some(pos + 1 - start);
        }
        if bytes[pos] == 0x1b && bytes.get(pos + 1) == Some(&b'\\') {
            return Some(pos + 2 - start);
        }
        pos += 1;
    }
    None
}
```

- [ ] **Step 4: Implement width utilities**

Replace `crates/pi-tui/src/utils/width.rs`:

```rust
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::ansi::ansi_sequence_len;

pub fn visible_width(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut clean = String::new();
    let mut pos = 0;

    while pos < text.len() {
        if let Some(len) = ansi_sequence_len(text, pos) {
            pos += len;
            continue;
        }

        let ch = text[pos..].chars().next().expect("pos is on a char boundary");
        if ch == '\t' {
            clean.push_str("   ");
        } else {
            clean.push(ch);
        }
        pos += ch.len_utf8();
    }

    clean.graphemes(true).map(UnicodeWidthStr::width).sum()
}

pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 || text.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mut width = 0;
    let mut pos = 0;

    while pos < text.len() {
        if let Some(len) = ansi_sequence_len(text, pos) {
            output.push_str(&text[pos..pos + len]);
            pos += len;
            continue;
        }

        let ch = text[pos..].chars().next().expect("pos is on a char boundary");
        if ch == '\t' {
            if width + 3 > max_width {
                break;
            }
            output.push(ch);
            width += 3;
            pos += ch.len_utf8();
            continue;
        }

        let mut graphemes = text[pos..].graphemes(true);
        let grapheme = graphemes.next().expect("grapheme exists");
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width + grapheme_width > max_width {
            break;
        }

        output.push_str(grapheme);
        width += grapheme_width;
        pos += grapheme.len();
    }

    output
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p pi-tui --test width`

Expected: PASS.

- [ ] **Step 6: Run crate tests**

Run: `cargo test -p pi-tui`

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/pi-tui/src/utils crates/pi-tui/tests/width.rs
git commit -m "feat(tui): add width utilities"
```

## Task 3: Implement Text, Spacer, and Container behavior

**Files:**
- Modify: `crates/pi-tui/src/component.rs`
- Modify: `crates/pi-tui/src/components/text.rs`
- Modify: `crates/pi-tui/src/components/spacer.rs`
- Test: `crates/pi-tui/tests/components.rs`

- [ ] **Step 1: Write failing component tests**

Create `crates/pi-tui/tests/components.rs`:

```rust
use pi_tui::{visible_width, Component, Container, Spacer, Text};

#[test]
fn spacer_renders_empty_lines() {
    let mut spacer = Spacer::new(3);
    assert_eq!(spacer.render(10), vec!["", "", ""]);
}

#[test]
fn container_renders_children_in_order() {
    let mut container = Container::new();
    container.add_child(Box::new(Text::new("alpha")));
    container.add_child(Box::new(Spacer::new(1)));
    container.add_child(Box::new(Text::new("beta")));

    assert_eq!(
        container.render(20),
        vec!["alpha".to_string(), "".to_string(), "beta".to_string()]
    );
}

#[test]
fn text_wraps_words_to_width() {
    let mut text = Text::new("alpha beta gamma");
    assert_eq!(
        text.render(10),
        vec!["alpha beta".to_string(), "gamma".to_string()]
    );
}

#[test]
fn text_splits_long_words_without_exceeding_width() {
    let mut text = Text::new("abcdefghij");
    let lines = text.render(4);

    assert_eq!(lines, vec!["abcd".to_string(), "efgh".to_string(), "ij".to_string()]);
    assert!(lines.iter().all(|line| visible_width(line) <= 4));
}

#[test]
fn text_handles_cjk_width_when_wrapping() {
    let mut text = Text::new("你好 world");
    let lines = text.render(6);

    assert_eq!(lines, vec!["你好".to_string(), "world".to_string()]);
    assert!(lines.iter().all(|line| visible_width(line) <= 6));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-tui --test components`

Expected: FAIL. `text_wraps_words_to_width`, `text_splits_long_words_without_exceeding_width`, and `text_handles_cjk_width_when_wrapping` should fail with the skeleton `Text` implementation.

- [ ] **Step 3: Implement Text wrapping**

Replace `crates/pi-tui/src/components/text.rs`:

```rust
use crate::{truncate_to_width, visible_width, Component};

pub struct Text {
    text: String,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Component for Text {
    fn render(&mut self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        let mut lines = Vec::new();
        for source_line in self.text.lines() {
            wrap_source_line(source_line, width, &mut lines);
        }
        if self.text.ends_with('\n') {
            lines.push(String::new());
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    }
}

fn wrap_source_line(source: &str, width: usize, lines: &mut Vec<String>) {
    let mut current = String::new();

    for word in source.split_whitespace() {
        if visible_width(word) > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            split_long_word(word, width, lines);
            continue;
        }

        if current.is_empty() {
            current.push_str(word);
            continue;
        }

        let candidate = format!("{} {}", current, word);
        if visible_width(&candidate) <= width {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    } else if source.is_empty() {
        lines.push(String::new());
    }
}

fn split_long_word(word: &str, width: usize, lines: &mut Vec<String>) {
    let mut rest = word.to_string();
    while !rest.is_empty() {
        let chunk = truncate_to_width(&rest, width);
        if chunk.is_empty() {
            break;
        }
        let consumed = chunk.len();
        lines.push(chunk);
        rest = rest[consumed..].to_string();
    }
}
```

- [ ] **Step 4: Keep Spacer implementation as-is**

Confirm `crates/pi-tui/src/components/spacer.rs` still contains:

```rust
use crate::Component;

pub struct Spacer {
    height: usize,
}

impl Spacer {
    pub fn new(height: usize) -> Self {
        Self { height }
    }
}

impl Component for Spacer {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![String::new(); self.height]
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p pi-tui --test components`

Expected: PASS.

- [ ] **Step 6: Run crate tests**

Run: `cargo test -p pi-tui`

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/pi-tui/src/component.rs crates/pi-tui/src/components crates/pi-tui/tests/components.rs
git commit -m "feat(tui): add basic components"
```

## Task 4: Implement terminal backends

**Files:**
- Modify: `crates/pi-tui/src/terminal.rs`
- Modify: `crates/pi-tui/src/virtual_terminal.rs`
- Test: `crates/pi-tui/tests/terminal.rs`

- [ ] **Step 1: Write failing terminal backend tests**

Create `crates/pi-tui/tests/terminal.rs`:

```rust
use pi_tui::{Terminal, TerminalOp, TerminalSize, VirtualTerminal};

#[test]
fn virtual_terminal_records_operations() {
    let mut terminal = VirtualTerminal::new(12, 4);

    terminal.hide_cursor().unwrap();
    terminal.write("hello").unwrap();
    terminal.move_by(-2).unwrap();
    terminal.clear_from_cursor().unwrap();
    terminal.flush().unwrap();

    assert_eq!(terminal.size(), TerminalSize { columns: 12, rows: 4 });
    assert_eq!(
        terminal.ops(),
        &[
            TerminalOp::HideCursor,
            TerminalOp::Write("hello".to_string()),
            TerminalOp::MoveBy(-2),
            TerminalOp::ClearFromCursor,
            TerminalOp::Flush,
        ]
    );
}

#[test]
fn virtual_terminal_can_resize_and_clear_ops() {
    let mut terminal = VirtualTerminal::new(12, 4);
    terminal.write("hello").unwrap();
    terminal.resize(20, 8);
    terminal.clear_ops();

    assert_eq!(terminal.size(), TerminalSize { columns: 20, rows: 8 });
    assert!(terminal.ops().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-tui --test terminal`

Expected: FAIL because `resize()` and `clear_ops()` do not exist yet.

- [ ] **Step 3: Implement ProcessTerminal and complete VirtualTerminal**

Replace `crates/pi-tui/src/terminal.rs`:

```rust
use std::io::{stdout, Write};

use crossterm::{
    cursor,
    execute,
    terminal::{self, Clear, ClearType},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: usize,
    pub rows: usize,
}

pub trait Terminal {
    fn size(&self) -> TerminalSize;
    fn write(&mut self, data: &str) -> std::io::Result<()>;
    fn move_by(&mut self, rows: i16) -> std::io::Result<()>;
    fn hide_cursor(&mut self) -> std::io::Result<()>;
    fn show_cursor(&mut self) -> std::io::Result<()>;
    fn clear_line(&mut self) -> std::io::Result<()>;
    fn clear_from_cursor(&mut self) -> std::io::Result<()>;
    fn clear_screen(&mut self) -> std::io::Result<()>;
    fn flush(&mut self) -> std::io::Result<()>;
}

pub struct ProcessTerminal;

impl ProcessTerminal {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal for ProcessTerminal {
    fn size(&self) -> TerminalSize {
        let (columns, rows) = terminal::size().unwrap_or((80, 24));
        TerminalSize {
            columns: columns as usize,
            rows: rows as usize,
        }
    }

    fn write(&mut self, data: &str) -> std::io::Result<()> {
        stdout().write_all(data.as_bytes())
    }

    fn move_by(&mut self, rows: i16) -> std::io::Result<()> {
        let mut out = stdout();
        if rows < 0 {
            execute!(out, cursor::MoveUp((-rows) as u16))?;
        } else if rows > 0 {
            execute!(out, cursor::MoveDown(rows as u16))?;
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        execute!(stdout(), cursor::Hide)
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        execute!(stdout(), cursor::Show)
    }

    fn clear_line(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::CurrentLine))
    }

    fn clear_from_cursor(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::FromCursorDown))
    }

    fn clear_screen(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        stdout().flush()
    }
}
```

Replace `crates/pi-tui/src/virtual_terminal.rs`:

```rust
use crate::{Terminal, TerminalSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalOp {
    Write(String),
    MoveBy(i16),
    HideCursor,
    ShowCursor,
    ClearLine,
    ClearFromCursor,
    ClearScreen,
    Flush,
}

pub struct VirtualTerminal {
    size: TerminalSize,
    ops: Vec<TerminalOp>,
}

impl VirtualTerminal {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            size: TerminalSize { columns, rows },
            ops: Vec::new(),
        }
    }

    pub fn resize(&mut self, columns: usize, rows: usize) {
        self.size = TerminalSize { columns, rows };
    }

    pub fn ops(&self) -> &[TerminalOp] {
        &self.ops
    }

    pub fn clear_ops(&mut self) {
        self.ops.clear();
    }

    pub fn written_output(&self) -> String {
        self.ops
            .iter()
            .filter_map(|op| match op {
                TerminalOp::Write(data) => Some(data.as_str()),
                _ => None,
            })
            .collect()
    }
}

impl Terminal for VirtualTerminal {
    fn size(&self) -> TerminalSize {
        self.size
    }

    fn write(&mut self, data: &str) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Write(data.to_string()));
        Ok(())
    }

    fn move_by(&mut self, rows: i16) -> std::io::Result<()> {
        self.ops.push(TerminalOp::MoveBy(rows));
        Ok(())
    }

    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::HideCursor);
        Ok(())
    }

    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ShowCursor);
        Ok(())
    }

    fn clear_line(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearLine);
        Ok(())
    }

    fn clear_from_cursor(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearFromCursor);
        Ok(())
    }

    fn clear_screen(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::ClearScreen);
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.ops.push(TerminalOp::Flush);
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pi-tui --test terminal`

Expected: PASS.

- [ ] **Step 5: Run crate tests**

Run: `cargo test -p pi-tui`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/pi-tui/src/terminal.rs crates/pi-tui/src/virtual_terminal.rs crates/pi-tui/tests/terminal.rs
git commit -m "feat(tui): add terminal backends"
```

## Task 5: Implement Tui full redraw and validation

**Files:**
- Modify: `crates/pi-tui/src/tui.rs`
- Test: `crates/pi-tui/tests/tui_render.rs`

- [ ] **Step 1: Write failing full redraw tests**

Create `crates/pi-tui/tests/tui_render.rs`:

```rust
use pi_tui::{
    Component, RenderStrategy, TerminalOp, Text, Tui, TuiError, VirtualTerminal,
};

struct RawComponent {
    lines: Vec<String>,
}

impl RawComponent {
    fn new(lines: &[&str]) -> Self {
        Self {
            lines: lines.iter().map(|line| line.to_string()).collect(),
        }
    }
}

impl Component for RawComponent {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.lines.clone()
    }
}

#[test]
fn first_render_uses_synchronized_full_redraw() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert_eq!(outcome.line_count, 1);
    assert_eq!(tui.full_redraws(), 1);
    assert!(tui.terminal().ops().contains(&TerminalOp::HideCursor));
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearScreen));
    assert!(tui.terminal().written_output().contains("\x1b[?2026h"));
    assert!(tui.terminal().written_output().contains("hello\x1b[0m\x1b]8;;\x07"));
    assert!(tui.terminal().written_output().contains("\x1b[?2026l"));
}

#[test]
fn line_too_wide_errors_before_writing() {
    let terminal = VirtualTerminal::new(4, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&["too wide"])));

    let err = tui.render_once().unwrap_err();

    match err {
        TuiError::LineTooWide {
            line_index,
            width,
            max_width,
            ..
        } => {
            assert_eq!(line_index, 0);
            assert_eq!(width, 8);
            assert_eq!(max_width, 4);
        }
        other => panic!("expected LineTooWide, got {other:?}"),
    }
    assert!(tui.terminal().ops().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-tui --test tui_render`

Expected: FAIL to compile because `add_child`, `render_once`, and `full_redraws` are missing.

- [ ] **Step 3: Implement full redraw rendering**

Replace `crates/pi-tui/src/tui.rs`:

```rust
use crate::{visible_width, Component, Terminal};

const SYNC_START: &str = "\x1b[?2026h";
const SYNC_END: &str = "\x1b[?2026l";
const LINE_RESET: &str = "\x1b[0m\x1b]8;;\x07";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStrategy {
    FullRedraw,
    Differential { first_changed_line: usize },
    NoChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOutcome {
    pub strategy: RenderStrategy,
    pub line_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line_index} is {width} columns wide, exceeding max width {max_width}: {line:?}")]
    LineTooWide {
        line_index: usize,
        width: usize,
        max_width: usize,
        line: String,
    },
}

pub struct Tui<T: Terminal> {
    terminal: T,
    children: Vec<Box<dyn Component>>,
    previous_lines: Vec<String>,
    previous_width: usize,
    previous_height: usize,
    cursor_row: usize,
    clear_on_shrink: bool,
    full_redraws: usize,
}

impl<T: Terminal> Tui<T> {
    pub fn new(terminal: T) -> Self {
        Self {
            terminal,
            children: Vec::new(),
            previous_lines: Vec::new(),
            previous_width: 0,
            previous_height: 0,
            cursor_row: 0,
            clear_on_shrink: true,
            full_redraws: 0,
        }
    }

    pub fn terminal(&self) -> &T {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut T {
        &mut self.terminal
    }

    pub fn add_child(&mut self, child: Box<dyn Component>) {
        self.children.push(child);
    }

    pub fn full_redraws(&self) -> usize {
        self.full_redraws
    }

    pub fn set_clear_on_shrink(&mut self, enabled: bool) {
        self.clear_on_shrink = enabled;
    }

    pub fn render_once(&mut self) -> Result<RenderOutcome, TuiError> {
        let size = self.terminal.size();
        let width = size.columns;
        let height = size.rows;
        let lines = self.render_lines(width);
        validate_lines(&lines, width)?;

        let strategy = self.choose_strategy(&lines, width, height);
        match strategy {
            RenderStrategy::NoChange => {}
            RenderStrategy::FullRedraw => self.render_full(&lines)?,
            RenderStrategy::Differential { first_changed_line } => {
                self.render_differential(&lines, first_changed_line)?
            }
        }

        self.previous_lines = lines.clone();
        self.previous_width = width;
        self.previous_height = height;
        self.cursor_row = lines.len().saturating_sub(1);

        Ok(RenderOutcome {
            strategy,
            line_count: lines.len(),
        })
    }

    fn render_lines(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for child in &mut self.children {
            lines.extend(child.render(width));
        }
        lines
    }

    fn choose_strategy(&self, lines: &[String], width: usize, height: usize) -> RenderStrategy {
        if self.previous_width == 0 || self.previous_height == 0 {
            return RenderStrategy::FullRedraw;
        }
        if self.previous_width != width || self.previous_height != height {
            return RenderStrategy::FullRedraw;
        }
        if self.clear_on_shrink && lines.len() < self.previous_lines.len() {
            return RenderStrategy::FullRedraw;
        }
        first_changed_line(&self.previous_lines, lines)
            .map(|first_changed_line| RenderStrategy::Differential { first_changed_line })
            .unwrap_or(RenderStrategy::NoChange)
    }

    fn render_full(&mut self, lines: &[String]) -> Result<(), TuiError> {
        self.full_redraws += 1;
        self.terminal.write(SYNC_START)?;
        self.terminal.hide_cursor()?;
        self.terminal.clear_screen()?;
        self.write_lines(lines)?;
        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        Ok(())
    }

    fn render_differential(
        &mut self,
        lines: &[String],
        first_changed_line: usize,
    ) -> Result<(), TuiError> {
        self.terminal.write(SYNC_START)?;
        let target = first_changed_line as i16;
        let current = self.cursor_row as i16;
        self.terminal.move_by(target - current)?;
        self.terminal.clear_from_cursor()?;
        self.write_lines(&lines[first_changed_line..])?;
        self.terminal.write(SYNC_END)?;
        self.terminal.flush()?;
        Ok(())
    }

    fn write_lines(&mut self, lines: &[String]) -> Result<(), TuiError> {
        for (index, line) in lines.iter().enumerate() {
            self.terminal.write(line)?;
            self.terminal.write(LINE_RESET)?;
            if index + 1 < lines.len() {
                self.terminal.write("\n")?;
            }
        }
        Ok(())
    }
}

fn validate_lines(lines: &[String], max_width: usize) -> Result<(), TuiError> {
    for (line_index, line) in lines.iter().enumerate() {
        let width = visible_width(line);
        if width > max_width {
            return Err(TuiError::LineTooWide {
                line_index,
                width,
                max_width,
                line: line.clone(),
            });
        }
    }
    Ok(())
}

fn first_changed_line(previous: &[String], next: &[String]) -> Option<usize> {
    let shared = previous.len().min(next.len());
    for index in 0..shared {
        if previous[index] != next[index] {
            return Some(index);
        }
    }
    if previous.len() != next.len() {
        Some(shared)
    } else {
        None
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pi-tui --test tui_render`

Expected: PASS.

- [ ] **Step 5: Run crate tests**

Run: `cargo test -p pi-tui`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/pi-tui/src/tui.rs crates/pi-tui/tests/tui_render.rs
git commit -m "feat(tui): add full redraw renderer"
```

## Task 6: Add differential rendering, resize, and shrink coverage

**Files:**
- Modify: `crates/pi-tui/tests/tui_render.rs`
- Modify: `crates/pi-tui/src/tui.rs`

- [ ] **Step 1: Add failing differential render tests**

Append to `crates/pi-tui/tests/tui_render.rs`:

```rust
#[test]
fn second_render_updates_from_first_changed_line_without_full_clear() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(RawComponent::new(&["header", "working", "footer"])));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent::new(&["header", "done", "footer"])));
    let outcome = tui.render_once().unwrap();

    assert_eq!(
        outcome.strategy,
        RenderStrategy::Differential {
            first_changed_line: 1
        }
    );
    assert!(!tui.terminal().ops().contains(&TerminalOp::ClearScreen));
    assert!(tui.terminal().ops().contains(&TerminalOp::MoveBy(-1)));
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearFromCursor));
    assert!(tui.terminal().written_output().contains("done"));
}

#[test]
fn width_change_triggers_full_redraw() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();
    tui.terminal_mut().resize(30, 5);

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearScreen));
}

#[test]
fn shrink_with_clear_on_shrink_triggers_full_redraw() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.set_clear_on_shrink(true);
    tui.add_child(Box::new(RawComponent::new(&["one", "two", "three"])));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    tui.clear_children();
    tui.add_child(Box::new(RawComponent::new(&["one"])));
    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::FullRedraw);
    assert!(tui.terminal().ops().contains(&TerminalOp::ClearScreen));
}

#[test]
fn unchanged_render_reports_no_change() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("hello")));
    tui.render_once().unwrap();
    tui.terminal_mut().clear_ops();

    let outcome = tui.render_once().unwrap();

    assert_eq!(outcome.strategy, RenderStrategy::NoChange);
    assert!(tui.terminal().ops().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pi-tui --test tui_render`

Expected: FAIL to compile because `clear_children()` is missing.

- [ ] **Step 3: Add child clearing API to Tui**

In `crates/pi-tui/src/tui.rs`, add this method inside `impl<T: Terminal> Tui<T>` after `add_child`:

```rust
    pub fn clear_children(&mut self) {
        self.children.clear();
    }
```

- [ ] **Step 4: Run test to verify behavior**

Run: `cargo test -p pi-tui --test tui_render`

Expected: PASS. If `second_render_updates_from_first_changed_line_without_full_clear` fails on `MoveBy(-1)`, inspect cursor accounting: after rendering three lines, `cursor_row` should be `2`, and the first changed line is `1`.

- [ ] **Step 5: Run crate tests**

Run: `cargo test -p pi-tui`

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/pi-tui/src/tui.rs crates/pi-tui/tests/tui_render.rs
git commit -m "feat(tui): add differential render coverage"
```

## Task 7: Add example and final verification

**Files:**
- Create: `crates/pi-tui/examples/render_once.rs`

- [ ] **Step 1: Create example**

Create `crates/pi-tui/examples/render_once.rs`:

```rust
use pi_tui::{ProcessTerminal, Text, Tui};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let terminal = ProcessTerminal::new();
    let mut tui = Tui::new(terminal);
    tui.add_child(Box::new(Text::new("pi-tui Rust renderer PoC")));
    tui.render_once()?;
    Ok(())
}
```

- [ ] **Step 2: Verify example builds**

Run: `cargo build -p pi-tui --example render_once`

Expected: PASS.

- [ ] **Step 3: Run crate tests**

Run: `cargo test -p pi-tui`

Expected: PASS.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/pi-tui/examples/render_once.rs
git commit -m "docs(tui): add render once example"
```

## Self-Review Checklist

- Spec coverage:
  - Width utilities: Task 2.
  - Component trait, Container, Text, Spacer: Tasks 1 and 3.
  - Terminal trait, VirtualTerminal, ProcessTerminal: Tasks 1 and 4.
  - Tui render manager, full redraw, differential redraw, shrink, resize, validation: Tasks 5 and 6.
  - Example and full verification: Task 7.
- No `ratatui` dependency is introduced.
- All behavior changes are test-first.
- All verification commands are local and offline.
