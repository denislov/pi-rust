# TUI-8 Color and Markdown Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add semantic 8-color styling and Markdown rendering polish to the interactive TUI transcript, giving it visual hierarchy without dominating the terminal or breaking the inline-owned-region renderer invariants.

**Architecture:** Three bottom-up layers. (1) New `pi-tui/src/style.rs` exposes `Color`, `Style`, `paint`, `paint_with`, `color_enabled`, and preset semantic constants; `paint` centralizes the NO_COLOR/TERM=dumb disable check via a cached `color_enabled()`. (2) The existing `Markdown` component rewrites its `markdown_to_lines` to emit ANSI-bearing strings (headings bold, inline code reverse, code blocks dim with fence rows, blockquotes dim). (3) `pi-coding-agent` interactive transcript rendering (`render_transcript_lines`, `render_tool_lines`, `footer`) applies per-role coloring. All downstream width/diff code is already ANSI-aware and unchanged.

**Tech Stack:** Rust edition 2024; existing `pi-tui` (`visible_width`/`truncate_to_width` already skip ANSI, `LINE_RESET` resets per line), existing `pulldown-cmark` + `unicode-segmentation` in `pi-tui`, existing `VirtualTerminal` test harness.

## Global Constraints

- Every ANSI-styled span emitted by `paint` is self-closing (ends with `\x1b[0m`), because differential rendering may rewrite a subset of rows.
- When `color_enabled() == false`, `paint`/`paint_with(..., false)` returns plain text (zero ANSI). All downstream code is unaware of the toggle.
- `Style` is `Copy` + `PartialEq` to ease test assertions and composition.
- Tests are deterministic and offline; no real provider key, no network, no real TTY. ANSI byte-level assertions use `paint_with(..., enabled: bool)` explicitly — never rely on the process-wide `color_enabled()` cache (it uses `OnceLock` and cannot be reset between tests).
- The literal footer substrings `status: idle` and `status: running` MUST remain (existing assertions). `paint` wraps text with SGR but does not alter visible characters, so substring assertions stay green.
- Run checks from `pi-rust/` (the Cargo workspace root): `cargo fmt --check`, `cargo test -p pi-tui`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace`.

## Reference: existing signatures the plan builds on

These already exist; do not re-implement them:

```rust
// crates/pi-tui/src/utils/width.rs (ANSI-aware, unchanged)
pub fn visible_width(text: &str) -> usize;       // skips ANSI sequences
pub fn truncate_to_width(text: &str, max_width: usize) -> String;  // preserves ANSI

// crates/pi-tui/src/tui.rs (unchanged)
const LINE_RESET: &str = "\x1b[0m\x1b]8;;\x07";  // appended per line by write_lines

// crates/pi-coding-agent/src/interactive/app.rs (current)
const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
fn render_transcript_lines(transcript: &Transcript, width: usize, max_tool_result_lines: usize) -> Vec<String>;
fn render_tool_lines(call_id: &str, name: &str, result: Option<&str>, is_error: bool, width: usize, max_tool_result_lines: usize) -> Vec<String>;
fn render_transcript_viewport(transcript: &Transcript, width: usize, viewport_rows: usize, max_tool_result_lines: usize) -> Vec<String>;
fn fit_line(line: &str, width: usize) -> String;
fn abbreviate_cwd(cwd: &Path) -> String;
fn format_tokens(count: u32) -> String;
fn welcome_line(keybindings: &KeybindingsManager) -> String;
```

`InteractiveRoot` fields currently include `status: InteractiveStatus`, `cwd: PathBuf`, `model_id: String`, `session_label: String`, `usage: (u32, u32)`, `tool_output_expanded: bool`, plus the action/submit/scroll plumbing.

## File Structure

- Create: `crates/pi-tui/src/style.rs` — `Color`, `Style`, `paint`, `paint_with`, `color_enabled`, preset semantic constants.
- Modify: `crates/pi-tui/src/lib.rs` — declare `pub mod style;` and re-export.
- Modify: `crates/pi-tui/src/components/markdown.rs` — rewrite `markdown_to_lines` and helpers to emit styled output.
- Create: `crates/pi-tui/tests/style.rs` — `paint_with`/preset unit tests.
- Modify: `crates/pi-tui/tests/markdown.rs` — update assertions for styled + degraded output.
- Modify: `crates/pi-coding-agent/src/interactive/app.rs` — color transcript/footer/tool rendering; thread a `color: bool` through internal render helpers; update in-file tests.
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs` — keep existing substring assertions (they still hold); no new ANSI assertions in scripted tests.

---

## Task 1: Style primitive — types and `paint_with`

**Files:**
- Create: `crates/pi-tui/src/style.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Create: `crates/pi-tui/tests/style.rs`

**Interfaces:**
- Consumes: none new.
- Produces:
  - `pub enum Color { Default, Red, Green, Yellow, Blue, Cyan, Magenta, White }`
  - `pub struct Style { fg, bg, bold, dim, reverse }` with `fg()`/`bold()`/`dim()`/`reverse()` chain constructors
  - `pub fn paint_with(text: &str, style: &Style, enabled: bool) -> String`
  - `pub fn paint(text: &str, style: &Style) -> String` (delegates to `paint_with` with `color_enabled()`)
  - `pub fn color_enabled() -> bool`
  - Preset constants: `USER`, `TOOL_NAME`, `ERROR`, `TOOL_ERROR`, `SYSTEM`, `STATUS_IDLE`, `STATUS_RUNNING`, `PATH`

- [ ] **Step 1: Write failing tests**

Create `crates/pi-tui/tests/style.rs`:

```rust
use pi_tui::{Color, Style, paint, paint_with};

#[test]
fn paint_with_disabled_returns_plain_text() {
    let style = Style::fg(Color::Red).bold();
    assert_eq!(paint_with("hi", &style, false), "hi");
}

#[test]
fn paint_with_enabled_single_fg() {
    let style = Style::fg(Color::Red);
    assert_eq!(paint_with("hi", &style, true), "\x1b[31mhi\x1b[0m");
}

#[test]
fn paint_with_enabled_bold_and_fg_merge_into_single_sgr() {
    let style = Style::fg(Color::Red).bold();
    assert_eq!(paint_with("hi", &style, true), "\x1b[1;31mhi\x1b[0m");
}

#[test]
fn paint_with_enabled_bold_reverse_and_fg_merge() {
    let style = Style::fg(Color::Red).bold().reverse();
    assert_eq!(paint_with("hi", &style, true), "\x1b[1;7;31mhi\x1b[0m");
}

#[test]
fn paint_with_default_color_emits_no_fg_sequence() {
    let style = Style::fg(Color::Default).bold();
    assert_eq!(paint_with("hi", &style, true), "\x1b[1mhi\x1b[0m");
}

#[test]
fn paint_with_default_style_emits_nothing() {
    let style = Style::default();
    assert_eq!(paint_with("hi", &style, true), "hi");
}

#[test]
fn paint_with_enabled_dim() {
    let style = Style::fg(Color::Default).dim();
    assert_eq!(paint_with("hi", &style, true), "\x1b[2mhi\x1b[0m");
}

#[test]
fn paint_with_enabled_bg() {
    let mut style = Style::default();
    style.bg = Color::Blue;
    assert_eq!(paint_with("hi", &style, true), "\x1b[44mhi\x1b[0m");
}

#[test]
fn paint_delegates_to_color_enabled() {
    // paint() uses color_enabled(); we cannot reset the OnceLock cache here,
    // so only assert that paint() output is one of the two valid forms.
    let style = Style::fg(Color::Red);
    let out = paint("hi", &style);
    assert!(out == "hi" || out == "\x1b[31mhi\x1b[0m", "unexpected paint output: {out:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui --test style
```

Expected: compile error — `Color`, `Style`, `paint`, `paint_with` are not exported from `pi_tui`.

- [ ] **Step 3: Implement `style.rs`**

Create `crates/pi-tui/src/style.rs`:

```rust
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    Red,
    Green,
    Yellow,
    Blue,
    Cyan,
    Magenta,
    White,
}

impl Color {
    fn fg_code(self) -> Option<u8> {
        match self {
            Color::Default => None,
            Color::Red => Some(1),
            Color::Green => Some(2),
            Color::Yellow => Some(3),
            Color::Blue => Some(4),
            Color::Magenta => Some(5),
            Color::Cyan => Some(6),
            Color::White => Some(7),
        }
    }

    fn bg_code(self) -> Option<u8> {
        self.fg_code().map(|n| n + 10)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub reverse: bool,
}

impl Style {
    pub const fn fg(color: Color) -> Self {
        Self {
            fg: color,
            bg: Color::Default,
            bold: false,
            dim: false,
            reverse: false,
        }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub const fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }

    fn has_any(&self) -> bool {
        self.fg != Color::Default
            || self.bg != Color::Default
            || self.bold
            || self.dim
            || self.reverse
    }
}

pub fn paint(text: &str, style: &Style) -> String {
    paint_with(text, style, color_enabled())
}

pub fn paint_with(text: &str, style: &Style, enabled: bool) -> String {
    if !enabled || !style.has_any() {
        return text.to_string();
    }

    let mut params: Vec<String> = Vec::new();
    if style.bold {
        params.push("1".to_string());
    }
    if style.dim {
        params.push("2".to_string());
    }
    if style.reverse {
        params.push("7".to_string());
    }
    if let Some(code) = style.fg.fg_code() {
        params.push(format!("3{code}"));
    }
    if let Some(code) = style.bg.bg_code() {
        params.push(format!("4{code}"));
    }

    format!("\x1b[{}m{}\x1b[0m", params.join(";"), text)
}

static CACHED: OnceLock<bool> = OnceLock::new();

pub fn color_enabled() -> bool {
    *CACHED.get_or_init(|| {
        !(std::env::var_os("NO_COLOR").is_some()
            || std::env::var("TERM").ok().as_deref() == Some("dumb"))
    })
}

pub const USER: Style = Style::fg(Color::Cyan);
pub const TOOL_NAME: Style = Style::fg(Color::Yellow);
pub const ERROR: Style = Style::fg(Color::Red).bold();
pub const TOOL_ERROR: Style = Style::fg(Color::Red);
pub const SYSTEM: Style = Style::fg(Color::Default).dim();
pub const STATUS_IDLE: Style = Style::fg(Color::Default).dim();
pub const STATUS_RUNNING: Style = Style::fg(Color::Yellow);
pub const PATH: Style = Style::fg(Color::Cyan);
```

- [ ] **Step 4: Wire the module into `lib.rs`**

In `crates/pi-tui/src/lib.rs`, the current module list has (in order): `component`, `components`, `cursor`, `input`, `kill_ring`, `overlay`, `runtime`, `terminal`, `tui`, `undo_stack`, `utils`, `virtual_terminal`, `word_navigation`.

Insert `style` between `runtime` and `terminal` (alphabetical). Change:

```rust
pub mod runtime;
pub mod terminal;
```

to:

```rust
pub mod runtime;
pub mod style;
pub mod terminal;
```

Then add a re-export line. After the existing `pub use runtime::RenderScheduler;` line, add:

```rust
pub use style::{
    Color, Style, paint, paint_with, color_enabled,
    ERROR, PATH, STATUS_IDLE, STATUS_RUNNING, SYSTEM, TOOL_ERROR, TOOL_NAME, USER,
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui --test style
```

Expected: PASS (9 tests).

- [ ] **Step 6: Verify the rest of the workspace still builds**

Run from `pi-rust/`:

```bash
cargo check --workspace
```

Expected: PASS (no errors).

- [ ] **Step 7: Commit**

```bash
cd pi-rust
git add crates/pi-tui/src/style.rs crates/pi-tui/src/lib.rs crates/pi-tui/tests/style.rs
git commit -m "feat(tui): add Style primitive with 8-color paint and NO_COLOR support"
```

---

## Task 2: Markdown — heading, inline code, blockquote, rule styling

**Files:**
- Modify: `crates/pi-tui/src/components/markdown.rs`
- Modify: `crates/pi-tui/tests/markdown.rs`

**Interfaces:**
- Consumes: `pi_tui::{paint, paint_with, Style, color_enabled}` (from Task 1).
- Produces: `markdown_to_lines` now emits ANSI-bearing strings when `color_enabled()` is true; degrades to plain text otherwise. `Markdown` struct public surface unchanged.

This task handles headings, inline code, blockquotes, and rules. Code blocks (fence rows + dim content) land in Task 3 because they require restructuring the `in_code_block` path.

- [ ] **Step 1: Write failing tests**

Replace the entire contents of `crates/pi-tui/tests/markdown.rs` with:

```rust
use pi_tui::{Color, Component, Markdown, Style, paint_with, visible_width};

fn bold() -> Style {
    Style::fg(Color::Default).bold()
}

fn reverse() -> Style {
    Style::default().reverse()
}

fn dim() -> Style {
    Style::fg(Color::Default).dim()
}

#[test]
fn markdown_renders_common_blocks() {
    let mut markdown = Markdown::new("# Title\n\n- one\n- two\n\n```rust\nfn main() {}\n```");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(joined.contains("Title"));
    assert!(joined.contains("one"));
    assert!(joined.contains("fn main() {}"));
}

#[test]
fn markdown_lines_do_not_exceed_width() {
    let mut markdown =
        Markdown::new("A long paragraph with **bold** text and `inline code` that must wrap.");
    for line in markdown.render(18) {
        assert!(visible_width(&line) <= 18, "line exceeded width: {:?}", line);
    }
}

#[test]
fn markdown_heading_is_bold_when_color_enabled() {
    let mut markdown = Markdown::new("# Title");
    let lines = markdown.render(40);
    let expected = paint_with("Title", &bold(), true);
    assert_eq!(lines, vec![expected]);
}

#[test]
fn markdown_inline_code_is_reverse_when_color_enabled() {
    let mut markdown = Markdown::new("see `code` here");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(
        joined.contains(&paint_with("code", &reverse(), true)),
        "expected reverse-styled inline code in: {joined:?}"
    );
}

#[test]
fn markdown_blockquote_is_dim_when_color_enabled() {
    let mut markdown = Markdown::new("> quoted text");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(
        joined.contains(&paint_with("> quoted text", &dim(), true)),
        "expected dim-styled blockquote in: {joined:?}"
    );
}

#[test]
fn markdown_rule_is_dim_when_color_enabled() {
    let mut markdown = Markdown::new("---");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    let dim_rule = paint_with(&"-".repeat(20), &dim(), true);
    assert!(
        joined.contains(&dim_rule),
        "expected dim-styled rule in: {joined:?}"
    );
}

#[test]
fn markdown_preserves_inline_punctuation_spacing() {
    let mut markdown = Markdown::new("A paragraph with **bold** text and `code`.");
    let lines = markdown.render(80);
    let joined = lines.join("\n");
    // The visible text (ignoring ANSI) must still read correctly.
    assert!(joined.contains("A paragraph with bold text and"));
    assert!(joined.contains(&paint_with("code", &reverse(), true)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui --test markdown
```

Expected: FAIL — `markdown_heading_is_bold_when_color_enabled` expects `\x1b[1mTitle\x1b[0m` but current output is plain `Title`. Other styling tests fail similarly. (`markdown_renders_common_blocks` and `markdown_lines_do_not_exceed_width` still pass because they assert substrings/width only.)

- [ ] **Step 3: Rewrite `markdown_to_lines` to track block context**

In `crates/pi-tui/src/components/markdown.rs`, the current `markdown_to_lines` signature and top of body are:

```rust
fn markdown_to_lines(text: &str, width: usize) -> Vec<String> {
    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut in_code_block = false;
    let mut list_depth = 0usize;
```

Replace the whole `markdown_to_lines` function and its helper `flush_current` with the version below. The key changes: a `BlockContext` struct tracks heading/quote/code flags and inline-code byte spans; `flush_current` applies `paint` based on context. Add the `use` import for `paint` at the top of the file.

First, add to the existing `use` at the top of the file. The current top is:

```rust
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

use crate::{Component, truncate_to_width, visible_width};
```

Change the `crate::{...}` line to import `Color`, `Style`, and `paint` (all re-exported from the crate root by Task 1):

```rust
use crate::{Color, Component, Style, paint, truncate_to_width, visible_width};
```

Then replace the `markdown_to_lines` function and `flush_current` helper (currently the two functions starting at `fn markdown_to_lines` and ending at the end of `fn flush_current`) with:

```rust
fn markdown_to_lines(text: &str, width: usize) -> Vec<String> {
    let parser = Parser::new_ext(text, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut context = BlockContext::default();
    let mut list_depth = 0usize;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => flush_current(&mut current, &mut blocks, &mut context),
            Event::Start(Tag::List(_)) => {
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                flush_current(&mut current, &mut blocks, &mut context);
            }
            Event::Start(Tag::Item) => {
                flush_current(&mut current, &mut blocks, &mut context);
                if list_depth > 0 {
                    current.push_str("- ");
                }
            }
            Event::End(TagEnd::Item) => flush_current(&mut current, &mut blocks, &mut context),
            Event::Start(Tag::BlockQuote(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_quote = true;
                current.push_str("> ");
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = false;
            }
            Event::Text(text) => {
                append_inline_text(&mut current, &text, context.in_code_block);
            }
            Event::Code(text) => {
                let start = current.len();
                current.push_str(&text);
                context.inline_code_spans.push((start, current.len()));
            }
            Event::SoftBreak => current.push(' '),
            Event::HardBreak => flush_current(&mut current, &mut blocks, &mut context),
            Event::Rule => {
                flush_current(&mut current, &mut blocks, &mut context);
                blocks.push(paint(&"-".repeat(width.min(20)), &Style::fg(Color::Default).dim()));
            }
            _ => {}
        }
    }
    flush_current(&mut current, &mut blocks, &mut context);

    let mut lines = Vec::new();
    for block in blocks {
        for source_line in block.split('\n') {
            wrap_line(source_line, width, &mut lines);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[derive(Default)]
struct BlockContext {
    heading: bool,
    in_quote: bool,
    in_code_block: bool,
    inline_code_spans: Vec<(usize, usize)>,
}

fn flush_current(current: &mut String, blocks: &mut Vec<String>, context: &mut BlockContext) {
    let block = current.trim_end();
    if block.is_empty() {
        current.clear();
        context.inline_code_spans.clear();
        context.heading = false;
        context.in_quote = false;
        return;
    }

    let styled = style_block(block, context);
    blocks.push(styled);

    current.clear();
    context.inline_code_spans.clear();
    context.heading = false;
    context.in_quote = false;
}

fn style_block(block: &str, context: &BlockContext) -> String {
    // Code blocks are handled in Task 3; for now pass through plain (Task 2 scope).
    if context.in_code_block {
        return block.to_string();
    }

    let with_inline = apply_inline_code(block, &context.inline_code_spans);
    if context.heading {
        return paint(&with_inline, &Style::fg(Color::Default).bold());
    }
    if context.in_quote {
        return paint(&with_inline, &Style::fg(Color::Default).dim());
    }
    with_inline
}

fn apply_inline_code(block: &str, spans: &[(usize, usize)]) -> String {
    if spans.is_empty() {
        return block.to_string();
    }
    let reverse_style = Style::default().reverse();
    let mut out = String::new();
    let mut cursor = 0usize;
    for &(start, end) in spans {
        // Spans are byte offsets into the original `current` before trim_end.
        // block is current.trim_end(), so spans may be clamped.
        let start = start.min(block.len());
        let end = end.min(block.len());
        if start > cursor {
            out.push_str(&block[cursor..start]);
        }
        if end > start {
            out.push_str(&paint(&block[start..end], &reverse_style));
        }
        cursor = end;
    }
    if cursor < block.len() {
        out.push_str(&block[cursor..]);
    }
    out
}
```

Note: `Color`, `Style`, and `paint` were already imported in the `crate::{...}` line at the top of the file (see Step 3 above). No additional import is needed.

- [ ] **Step 4: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui --test markdown
```

Expected: PASS (7 tests). The `markdown_renders_common_blocks` test still passes because it only checks substrings; the code block content is plain text (Task 3 adds fence rows), and `fn main() {}` is still present.

- [ ] **Step 5: Run the full pi-tui suite to check for regressions**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui
```

Expected: PASS (all existing tests green; the `tui_render`/`editor`/etc. tests do not depend on Markdown output).

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-tui/src/components/markdown.rs crates/pi-tui/tests/markdown.rs
git commit -m "feat(tui): style markdown headings, inline code, blockquotes, rules"
```

---

## Task 3: Markdown — fenced code block with dim fence rows and dim content

**Files:**
- Modify: `crates/pi-tui/src/components/markdown.rs`
- Modify: `crates/pi-tui/tests/markdown.rs`

**Interfaces:**
- Consumes: `pi_tui::{paint, Style, Color}` (from Task 1), `BlockContext` (from Task 2).
- Produces: code blocks now render as a dim ```` ``` ```` fence row, dim indented content rows, and a closing dim fence row.

- [ ] **Step 1: Write failing tests**

Append to `crates/pi-tui/tests/markdown.rs` (after the existing tests):

```rust
#[test]
fn markdown_code_block_has_dim_fence_rows_and_dim_content() {
    let mut markdown = Markdown::new("```rust\nfn main() {}\n```");
    let lines = markdown.render(40);
    let joined = lines.join("\n");

    let dim_fence = paint_with("```", &dim(), true);
    let dim_content = paint_with("   fn main() {}", &dim(), true);

    assert!(
        joined.contains(&dim_fence),
        "expected dim fence row in: {joined:?}"
    );
    assert!(
        joined.contains(&dim_content),
        "expected dim indented content in: {joined:?}"
    );
    // Two fence rows (open + close).
    assert_eq!(
        joined.matches(&dim_fence).count(),
        2,
        "expected two fence rows in: {joined:?}"
    );
}

#[test]
fn markdown_code_block_multiline_content_each_line_indented_and_dim() {
    let mut markdown = Markdown::new("```\nlet a = 1;\nlet b = 2;\n```");
    let lines = markdown.render(40);
    let joined = lines.join("\n");
    assert!(
        joined.contains(&paint_with("   let a = 1;", &dim(), true)),
        "expected dim indented first line in: {joined:?}"
    );
    assert!(
        joined.contains(&paint_with("   let b = 2;", &dim(), true)),
        "expected dim indented second line in: {joined:?}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui --test markdown
```

Expected: FAIL — `markdown_code_block_has_dim_fence_rows_and_dim_content` expects dim fence rows and dim indented content, but current code-block path (from Task 2's `style_block`) returns plain `block.to_string()` for code blocks, and `markdown_to_lines` does not emit fence rows.

- [ ] **Step 3: Rewrite the code-block path to emit fence rows and dim content**

In `crates/pi-tui/src/components/markdown.rs`, the `style_block` function (from Task 2) currently has:

```rust
fn style_block(block: &str, context: &BlockContext) -> String {
    // Code blocks are handled in Task 3; for now pass through plain (Task 2 scope).
    if context.in_code_block {
        return block.to_string();
    }
    ...
}
```

The problem: a single code block contains multiple lines separated by `\n`, and `markdown_to_lines` later splits each block by `\n` and word-wraps. For code blocks we must NOT word-wrap (preserve formatting) and must emit fence rows + dim each line. The cleanest fix is to special-case code blocks in `markdown_to_lines` so they push their rows directly into `blocks` as already-styled, already-split lines, and `style_block` never sees them.

Change `markdown_to_lines`: the `Event::Start(Tag::CodeBlock(_))` and `Event::End(TagEnd::CodeBlock)` arms and the `Event::Text(text)` arm need updating. The current (Task 2) arms are:

```rust
            Event::Start(Tag::CodeBlock(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = false;
            }
            Event::Text(text) => {
                append_inline_text(&mut current, &text, context.in_code_block);
            }
```

Replace these three arms with:

```rust
            Event::Start(Tag::CodeBlock(_)) => {
                flush_current(&mut current, &mut blocks, &mut context);
                context.in_code_block = true;
                blocks.push(paint("```", &Style::fg(Color::Default).dim()));
            }
            Event::End(TagEnd::CodeBlock) => {
                // Flush accumulated code text as dim indented lines, then close fence.
                let code = current.trim_end();
                for source_line in code.split('\n') {
                    let line = if source_line.is_empty() {
                        paint("   ", &Style::fg(Color::Default).dim())
                    } else {
                        paint(&format!("   {source_line}"), &Style::fg(Color::Default).dim())
                    };
                    blocks.push(line);
                }
                current.clear();
                context.in_code_block = false;
                blocks.push(paint("```", &Style::fg(Color::Default).dim()));
            }
            Event::Text(text) => {
                if context.in_code_block {
                    current.push_str(&text);
                } else {
                    append_inline_text(&mut current, &text, false);
                }
            }
```

Then in `style_block`, remove the now-unreachable code-block arm (it can stay as a defensive no-op, but since `in_code_block` is only true while accumulating into `current` and we never call `flush_current` while it's true, the arm is dead code). Change `style_block` to:

```rust
fn style_block(block: &str, context: &BlockContext) -> String {
    // Code blocks are emitted directly in markdown_to_lines (fence rows + dim lines),
    // so this function only handles headings, quotes, and plain paragraphs.
    let with_inline = apply_inline_code(block, &context.inline_code_spans);
    if context.heading {
        return paint(&with_inline, &Style::fg(Color::Default).bold());
    }
    if context.in_quote {
        return paint(&with_inline, &Style::fg(Color::Default).dim());
    }
    with_inline
}
```

Also, the final loop in `markdown_to_lines` splits each block by `\n` and calls `wrap_line`. Code-block lines are already single lines (no `\n` inside, since we split on `\n` when pushing them), but they contain ANSI. `wrap_line` calls `visible_width` which skips ANSI, so wrapping is safe. However, code lines should NOT be word-wrapped (they may exceed width and that's intentional for code). Add a guard: if a block line already starts with the dim fence or dim indent, skip wrapping and push as-is. The simplest check is whether the block line contains the dim SGR `\x1b[2m`. Change the final loop:

```rust
    let mut lines = Vec::new();
    for block in blocks {
        if block.contains("\x1b[2m") {
            // Pre-styled code-block line; do not word-wrap.
            lines.push(block);
            continue;
        }
        for source_line in block.split('\n') {
            wrap_line(source_line, width, &mut lines);
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui --test markdown
```

Expected: PASS (9 tests). The two new code-block tests pass; existing tests still pass (`markdown_renders_common_blocks` asserts `fn main() {}` substring which is still present inside the dim content).

- [ ] **Step 5: Run the full pi-tui suite**

Run from `pi-rust/`:

```bash
cargo test -p pi-tui
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd pi-rust
git add crates/pi-tui/src/components/markdown.rs crates/pi-tui/tests/markdown.rs
git commit -m "feat(tui): render markdown code blocks with dim fence rows and content"
```

---

## Task 4: Transcript — thread `color` flag and color tool/user/system/error rows

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Modify: `crates/pi-coding-agent/tests/interactive_mode.rs` (no new ANSI assertions; keep existing substring assertions)

**Interfaces:**
- Consumes: `pi_tui::{paint_with, Style, ERROR, PATH, SYSTEM, STATUS_IDLE, STATUS_RUNNING, TOOL_ERROR, TOOL_NAME, USER, color_enabled}` (from Task 1).
- Produces: `render_transcript_lines`, `render_tool_lines`, `render_transcript_viewport` gain a `color: bool` parameter; `footer` uses `paint_with` with `color_enabled()`; the `InteractiveRoot::render` and `apply_events` call sites pass `color_enabled()`.

- [ ] **Step 1: Update the in-file unit test to assert ANSI bytes**

In `crates/pi-coding-agent/src/interactive/app.rs`, the existing `#[cfg(test)] mod tests` block has `render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output` which asserts plain-text vectors. Replace that whole test function with the version below, which threads `color: bool` explicitly and asserts both colored and uncolored forms. Also add a new error-item test.

The current function (lines ~1046-1088) is:

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
    }
```

Replace it with:

```rust
    #[test]
    fn render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output() {
        use pi_tui::{paint_with, STATUS_IDLE, STATUS_RUNNING, SYSTEM, TOOL_NAME};
        let yellow = |s: &str| paint_with(s, &TOOL_NAME, true);
        let dim = |s: &str| paint_with(s, &SYSTEM, true);
        let running = |s: &str| paint_with(s, &STATUS_RUNNING, true);
        let idle = |s: &str| paint_with(s, &STATUS_IDLE, true);

        let mut transcript = Transcript::new();
        transcript.apply_event(UiEvent::ToolStarted {
            call_id: "tool_1".to_string(),
            name: "read".to_string(),
            args: serde_json::Value::Null,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, true),
            vec![format!(
                "{} {} tool_1 {}",
                yellow("tool"),
                yellow("read"),
                running("running")
            )]
        );
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, false),
            vec!["tool read tool_1 running"]
        );

        transcript.apply_event(UiEvent::ToolFinished {
            call_id: "tool_1".to_string(),
            result: "line 1\nline 2\nline 3\nline 4\nline 5".to_string(),
            is_error: false,
        });

        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, true),
            vec![
                format!("{} {} tool_1 {}", yellow("tool"), yellow("read"), idle("done")),
                "line 1".to_string(),
                "line 2".to_string(),
                "line 3".to_string(),
                dim("... truncated 2 lines"),
            ]
        );
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, false),
            vec![
                "tool read tool_1 done",
                "line 1",
                "line 2",
                "line 3",
                "... truncated 2 lines",
            ]
        );

        assert_eq!(
            render_transcript_lines(&transcript, 80, 20, true),
            vec![
                format!("{} {} tool_1 {}", yellow("tool"), yellow("read"), idle("done")),
                "line 1".to_string(),
                "line 2".to_string(),
                "line 3".to_string(),
                "line 4".to_string(),
                "line 5".to_string(),
            ]
        );
    }

    #[test]
    fn render_transcript_lines_colors_error_item_red_bold() {
        use pi_tui::{paint_with, ERROR};
        let red_bold = |s: &str| paint_with(s, &ERROR, true);
        let mut transcript = Transcript::new();
        transcript.push(TranscriptItem::Error {
            text: "boom".to_string(),
        });
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, true),
            vec![format!("{}: {}", red_bold("error"), red_bold("boom"))]
        );
        assert_eq!(
            render_transcript_lines(&transcript, 80, 3, false),
            vec!["error: boom"]
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib render_transcript_lines
```

Expected: compile error — `render_transcript_lines` takes 3 args, not 4.

- [ ] **Step 3: Update imports, `render_transcript_lines`, and `render_tool_lines`**

In `crates/pi-coding-agent/src/interactive/app.rs`, the current `pi_tui` import block (lines ~9-13) is:

```rust
use pi_tui::{
    Component, Editor, InputEvent, KeybindingsManager, Markdown, ProcessTerminal, RenderScheduler,
    StdinBuffer, TUI_KEYBINDINGS, Terminal, Tui, TuiError, is_key_release, matches_key,
    truncate_to_width, visible_width,
};
```

Replace with:

```rust
use pi_tui::{
    ERROR, PATH, STATUS_IDLE, STATUS_RUNNING, SYSTEM, TOOL_ERROR, TOOL_NAME, USER,
    Component, Editor, InputEvent, KeybindingsManager, Markdown, ProcessTerminal,
    RenderScheduler, StdinBuffer, Style, TUI_KEYBINDINGS, Terminal, Tui, TuiError,
    color_enabled, is_key_release, matches_key, paint_with, truncate_to_width, visible_width,
};
```

Now replace `render_transcript_lines` (lines ~893-929). The current function body is:

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

Replace with:

```rust
fn render_transcript_lines(
    transcript: &Transcript,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    transcript
        .items()
        .iter()
        .flat_map(|item| match item {
            TranscriptItem::User { text } => {
                vec![fit_line(
                    &format!("{}: {}", paint_with("user", &USER, color), text),
                    width,
                )]
            }
            TranscriptItem::System { text } => {
                vec![fit_line(&paint_with(text, &SYSTEM, color), width)]
            }
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
                color,
            ),
            TranscriptItem::Error { text } => {
                vec![fit_line(
                    &format!(
                        "{}: {}",
                        paint_with("error", &ERROR, color),
                        paint_with(text, &ERROR, color)
                    ),
                    width,
                )]
            }
        })
        .collect()
}
```

Now replace `render_tool_lines` (lines ~931-961). The current function is:

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

Replace with:

```rust
fn render_tool_lines(
    call_id: &str,
    name: &str,
    result: Option<&str>,
    is_error: bool,
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    let status = match (result, is_error) {
        (None, _) => "running",
        (Some(_), true) => "error",
        (Some(_), false) => "done",
    };
    let status_style = match status {
        "running" => STATUS_RUNNING,
        "error" => TOOL_ERROR,
        "done" => STATUS_IDLE,
        _ => Style::default(),
    };
    let header = format!(
        "{} {} {} {}",
        paint_with("tool", &TOOL_NAME, color),
        paint_with(name, &TOOL_NAME, color),
        call_id,
        paint_with(status, &status_style, color),
    );
    let mut lines = vec![fit_line(&header, width)];
    let Some(result) = result else {
        return lines;
    };

    let result_lines = result.lines().collect::<Vec<_>>();
    lines.extend(result_lines.iter().take(max_tool_result_lines).map(|line| {
        if is_error {
            fit_line(&paint_with(line, &TOOL_ERROR, color), width)
        } else {
            fit_line(line, width)
        }
    }));
    let omitted = result_lines.len().saturating_sub(max_tool_result_lines);
    if omitted > 0 {
        lines.push(fit_line(
            &paint_with(&format!("... truncated {omitted} lines"), &SYSTEM, color),
            width,
        ));
    }
    lines
}
```

- [ ] **Step 4: Update `render_transcript_viewport` and its call sites**

The current `render_transcript_viewport` (lines ~963-992) is:

```rust
fn render_transcript_viewport(
    transcript: &Transcript,
    width: usize,
    viewport_rows: usize,
    max_tool_result_lines: usize,
) -> Vec<String> {
    let lines = render_transcript_lines(transcript, width, max_tool_result_lines);
    if lines.len() <= viewport_rows {
        let mut padded = lines;
        while padded.len() < viewport_rows {
            padded.push(String::new());
        }
        return padded;
    }

    let max_scroll_offset = lines.len().saturating_sub(1);
    let scroll_offset = transcript.scroll_offset().min(max_scroll_offset);
    let bottom = lines.len().saturating_sub(scroll_offset);
    let top = bottom.saturating_sub(viewport_rows);
    let mut visible = lines[top..bottom].to_vec();
    while visible.len() < viewport_rows {
        visible.insert(0, String::new());
    }
    if transcript.has_new_output_below() && !visible.is_empty() {
        let indicator = fit_line("... new output below", width);
        let last = visible.len() - 1;
        visible[last] = indicator;
    }
    visible
}
```

Replace with (adding `color: bool`, threading through, and coloring the indicator):

```rust
fn render_transcript_viewport(
    transcript: &Transcript,
    width: usize,
    viewport_rows: usize,
    max_tool_result_lines: usize,
    color: bool,
) -> Vec<String> {
    let lines = render_transcript_lines(transcript, width, max_tool_result_lines, color);
    if lines.len() <= viewport_rows {
        let mut padded = lines;
        while padded.len() < viewport_rows {
            padded.push(String::new());
        }
        return padded;
    }

    let max_scroll_offset = lines.len().saturating_sub(1);
    let scroll_offset = transcript.scroll_offset().min(max_scroll_offset);
    let bottom = lines.len().saturating_sub(scroll_offset);
    let top = bottom.saturating_sub(viewport_rows);
    let mut visible = lines[top..bottom].to_vec();
    while visible.len() < viewport_rows {
        visible.insert(0, String::new());
    }
    if transcript.has_new_output_below() && !visible.is_empty() {
        let indicator = fit_line(&paint_with("... new output below", &SYSTEM, color), width);
        let last = visible.len() - 1;
        visible[last] = indicator;
    }
    visible
}
```

Now update all call sites. There are three places that call `render_transcript_lines` or `render_transcript_viewport`:

**Call site A: `InteractiveRoot::render` (lines ~324-329).** The current code is:

```rust
        let mut lines = render_transcript_viewport(
            &self.transcript,
            width,
            transcript_rows,
            max_tool_result_lines,
        );
```

Replace with:

```rust
        let mut lines = render_transcript_viewport(
            &self.transcript,
            width,
            transcript_rows,
            max_tool_result_lines,
            color_enabled(),
        );
```

**Call site B: `apply_events` (lines ~243, ~257).** The current code calls `render_transcript_lines` twice to compute row counts for scroll preservation:

```rust
        let previous_rows = if previous_scroll_offset > 0 {
            render_transcript_lines(&self.transcript, self.viewport_width, MAX_TOOL_RESULT_LINES).len()
        } else {
            0
        };
```

and:

```rust
            let current_rows =
                render_transcript_lines(&self.transcript, self.viewport_width, MAX_TOOL_RESULT_LINES).len();
```

Replace both with the `color_enabled()` argument:

```rust
        let previous_rows = if previous_scroll_offset > 0 {
            render_transcript_lines(
                &self.transcript,
                self.viewport_width,
                MAX_TOOL_RESULT_LINES,
                color_enabled(),
            )
            .len()
        } else {
            0
        };
```

and:

```rust
            let current_rows = render_transcript_lines(
                &self.transcript,
                self.viewport_width,
                MAX_TOOL_RESULT_LINES,
                color_enabled(),
            )
            .len();
```

- [ ] **Step 5: Color the footer**

The current `footer` method (lines ~274-294) is:

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
            parts.push(format!(
                "↑{} ↓{}",
                format_tokens(self.usage.0),
                format_tokens(self.usage.1)
            ));
        }
        parts.join(" | ")
    }
```

Replace with (coloring status label+value, cwd value, and usage):

```rust
    fn footer(&self) -> String {
        let color = color_enabled();
        let status_str = match self.status {
            InteractiveStatus::Idle => "idle",
            InteractiveStatus::Running => "running",
        };
        let status_style = match self.status {
            InteractiveStatus::Idle => STATUS_IDLE,
            InteractiveStatus::Running => STATUS_RUNNING,
        };
        let cwd = abbreviate_cwd(&self.cwd);
        let mut parts = vec![
            format!(
                "{}: {}",
                paint_with("status", &status_style, color),
                paint_with(status_str, &status_style, color)
            ),
            format!("cwd: {}", paint_with(&cwd, &PATH, color)),
            format!("model: {}", self.model_id),
            format!("session: {}", self.session_label),
        ];
        if self.usage != (0, 0) {
            parts.push(paint_with(
                &format!(
                    "↑{} ↓{}",
                    format_tokens(self.usage.0),
                    format_tokens(self.usage.1)
                ),
                &SYSTEM,
                color,
            ));
        }
        parts.join(" | ")
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib render_transcript_lines
cargo test -p pi-coding-agent --test interactive_mode
```

Expected: PASS. The in-file unit tests pass (they use `paint_with(..., true)` and `paint_with(..., false)` explicitly). The scripted `interactive_mode` tests pass because `paint_with` does not alter visible characters — `frame.contains("status: idle")`, `frame.contains("↑")`, `frame.contains("pi · ")`, `frame.contains("> typed")` all still hold (ANSI wraps but does not change the substrings).

- [ ] **Step 7: Run the full pi-coding-agent suite**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS (all existing tests green; the `ctrl_o_toggles_tool_output_expansion_in_root` test asserts `collapsed.contains("... truncated")` which still holds because `paint_with` preserves the text).

- [ ] **Step 8: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs
git commit -m "feat(interactive): color transcript roles, tool rows, and footer"
```

---

## Task 5: Final verification

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
git log --oneline -6
```

Expected: the five task commits sit on top of the spec commit (`862a3b5 docs: add TUI-8 color and Markdown polish design`), with clean, focused messages:
1. `feat(tui): add Style primitive with 8-color paint and NO_COLOR support`
2. `feat(tui): style markdown headings, inline code, blockquotes, rules`
3. `feat(tui): render markdown code blocks with dim fence rows and content`
4. `feat(interactive): color transcript roles, tool rows, and footer`
5. (no commit for Task 5 unless a fix was needed)

- [ ] **Step 5: NO_COLOR degradation smoke (optional, host-dependent)**

From `pi-rust/`, verify the NO_COLOR path degrades to plain text:

```bash
NO_COLOR=1 cargo test -p pi-tui --test markdown
```

Expected: tests that assert `paint_with(..., true)` still pass (they bypass `color_enabled`). This confirms the `paint_with` indirection works regardless of the environment. (The `Markdown` component itself calls `paint` which reads `color_enabled()`; under `NO_COLOR=1` the cache initializes to `false` and Markdown degrades. But since the tests assert `paint_with(..., true)` explicitly, they are independent of the cache. This step is a sanity check, not a regression gate.)
