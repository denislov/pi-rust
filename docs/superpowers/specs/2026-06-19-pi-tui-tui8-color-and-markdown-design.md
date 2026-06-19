# Design: pi-tui TUI-8 color and Markdown polish

- Date: 2026-06-19
- Status: Draft (pending review)
- Scope: First slice of TUI-8 from `docs/TUI_INTERACTION_ROADMAP.md` — semantic 8-color styling and Markdown rendering enhancement.
- Depends on: `pi-tui` rendering foundation (Inline surface, ANSI-aware `visible_width`/`truncate_to_width`, `RenderScheduler`), `pi-coding-agent` interactive transcript rendering.

## 1. Context

`docs/TUI_INTERACTION_ROADMAP.md` milestones TUI-1 through TUI-6 are substantively complete: inline owned-region rendering, cursor stability, render scheduling, transcript layout/scrolling, and terminal lifecycle all have green tests. TUI-8 ("Interaction Polish After Stability") remains open. The current interactive transcript renders as pure text — no color, no visual hierarchy. The `Markdown` component (`crates/pi-tui/src/components/markdown.rs`) parses pulldown-cmark events but emits plain strings: headings are indistinguishable from paragraphs, inline code has no emphasis, code blocks have no fence, blockquotes are only a `> ` prefix.

This spec ports the first coherent slice of TUI-8: **semantic 8-color styling** for transcript roles plus **Markdown rendering enhancement** (standard set). Together they give the interactive transcript visual hierarchy without dominating the terminal or violating the inline-owned-region renderer invariants.

Behavioral reference (TS): `pi/packages/tui/src/components/` and `pi/packages/coding-agent/src/core/tools/*.ts` render-call paths. The TS side uses a richer theme system; this Rust slice intentionally stays at 8-color semantic mapping (see Non-goals).

## 2. Goals and success criteria

Build a `Style` primitive in `pi-tui`, rewrite the `Markdown` component to emit styled output, and wire semantic coloring into the interactive transcript and footer.

Done when:

1. `cargo fmt --check`, `cargo test -p pi-tui`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, and `cargo check --workspace` pass from `pi-rust/`.
2. `pi-tui` exposes `Color`, `Style`, `paint`, `paint_with`, and `color_enabled` as public API.
3. `paint(text, &style)` emits 8-color ANSI SGR sequences when color is enabled and returns plain text when disabled.
4. `color_enabled()` returns `false` when `NO_COLOR` is set (any value) or `TERM=dumb`; result is cached per process via `OnceLock`.
5. The `Markdown` component renders headings bold, inline code reverse, fenced code blocks with a dim fence row and dim indented content, and blockquote lines dim — when color is enabled. When disabled, output degrades to plain text (fence rows remain, but without dim SGR).
6. The interactive transcript (`render_transcript_lines`, `render_tool_lines`, `footer`, welcome, "new output below" indicator) applies semantic 8-color styling per the role table in section 4.4.
7. Existing substring assertions in `crates/pi-coding-agent/tests/interactive_mode.rs` (`status: idle`, `↑`, `↓`, `> typed`, `pi · `) continue to pass — `paint` wraps text with SGR but does not alter the visible characters.
8. The offline test suite passes with no network access and no credentials. ANSI byte-level assertions use `paint_with(..., enabled: bool)` directly; scripted interactive tests only assert text substrings to avoid TTY-dependent `color_enabled` caching.

## 3. Non-goals (this increment)

- Spinner/progress animation for running agent/tools (later TUI-8 slice).
- `SelectList`-based model/session/status menus (later TUI-8 slice).
- Theme system (dark/light/custom palettes), 256-color or true-color output.
- OSC 8 hyperlinks in Markdown links.
- Table alignment styling beyond current pass-through.
- Heading-level color differentiation (H1 blue, H2 cyan, etc.) — all headings use bold only this slice.
- CLI flags (`--color=always|auto|never`) or settings-driven color toggle.
- Changes to `pi-tui`'s `Terminal` trait, `Tui` renderer, or render scheduling.
- Changes to `pi-agent-core` or `pi-ai`.

## 4. Design

### 4.1 Architecture and data flow

Three layers, bottom-up, single-direction dependency:

    pi-tui/src/style.rs                    [new] Color, Style, paint(), color_enabled()
            ^
            | uses
    pi-tui/src/components/markdown.rs      [rewrite render] emits ANSI-bearing String
            ^
            | uses
    pi-coding-agent/src/interactive/app.rs [color render_transcript_lines etc.] per-role coloring

**Data flow.** Each component's `render(width) -> Vec<String>` emits lines that already contain ANSI (produced by `paint`). These lines flow into `Tui::render_once()`, whose `visible_width` and `truncate_to_width` are already ANSI-aware (they skip escape sequences when measuring and preserve them when truncating). Differential rendering is unchanged. `LINE_RESET` (`\x1b[0m\x1b]8;;\x07`) is still appended by `write_lines` at the end of every written line, ensuring styles do not leak across lines.

**Key invariants:**

- Every ANSI-styled span emitted by `paint` is self-closing (ends with `\x1b[0m` or equivalent reset), because differential rendering may rewrite a subset of rows.
- When `color_enabled() == false`, `paint` returns plain text (zero ANSI). All downstream code is unaware of the toggle.
- `Style` is a value type (`Copy` + `PartialEq`) to ease test assertions and composition.

**NO_COLOR / TERM=dumb detection.** Checked once at the `paint` entry point via a cached `color_enabled()` (uses `std::sync::OnceLock`). Logic: `env::var_os("NO_COLOR").is_some()` OR `env::var("TERM").ok().as_deref() == Some("dumb")` -> disabled.

### 4.2 Style API (`pi-tui/src/style.rs`, new file)

Public types and functions:

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Color {
        #[default]
        Default,
        Red, Green, Yellow, Blue, Cyan, Magenta, White,
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
        pub const fn fg(color: Color) -> Self;   // fg=color, others default
        pub const fn bold(self) -> Self;         // chain bold=true
        pub const fn dim(self) -> Self;          // chain dim=true
        pub const fn reverse(self) -> Self;      // chain reverse=true
    }

    pub fn paint(text: &str, style: &Style) -> String;
    pub fn paint_with(text: &str, style: &Style, enabled: bool) -> String;
    pub fn color_enabled() -> bool;

- `paint` wraps `text` with `style`, returning an ANSI-bearing string. Returns plain text when `color_enabled()` is false.
- `paint_with` is the same as `paint` but with an explicit `enabled` flag. Used by tests to assert ANSI output deterministically without depending on the process-wide `color_enabled()` cache.
- `color_enabled` returns whether this process should emit color. Cached on first call.

**ANSI generation rules (8-color standard):**

- fg: `\x1b[3{n}m` for n in 1..=7 mapping Red..White. `Color::Default` emits no fg sequence.
- bg: `\x1b[4{n}m` similarly. `Color::Default` emits no bg sequence.
- bold: `\x1b[1m`, dim: `\x1b[2m`, reverse: `\x1b[7m`.
- Multiple modifiers merge into a single SGR sequence: e.g. bold + red fg -> `\x1b[1;31m`.
- Every `paint` result ends with `\x1b[0m`.
- `Style::default()` (all default) with enabled=true returns plain text (nothing to wrap).

**`color_enabled()` implementation:**

    static CACHED: OnceLock<bool> = OnceLock::new();
    pub fn color_enabled() -> bool {
        *CACHED.get_or_init(|| {
            !(std::env::var_os("NO_COLOR").is_some()
                || std::env::var("TERM").ok().as_deref() == Some("dumb"))
        })
    }

`paint` delegates to `paint_with(text, style, color_enabled())`.

**Preset semantic constants** (bottom of `style.rs`, shared by Markdown and transcript to keep semantics consistent):

    pub const USER: Style = Style::fg(Color::Cyan);
    pub const TOOL_NAME: Style = Style::fg(Color::Yellow);
    pub const ERROR: Style = Style::fg(Color::Red).bold();
    pub const TOOL_ERROR: Style = Style::fg(Color::Red);
    pub const SYSTEM: Style = Style::fg(Color::Default).dim();
    pub const STATUS_IDLE: Style = Style::fg(Color::Default).dim();
    pub const STATUS_RUNNING: Style = Style::fg(Color::Yellow);
    pub const PATH: Style = Style::fg(Color::Cyan);

**`lib.rs` exports:** `pub mod style; pub use style::{Color, Style, paint, paint_with, color_enabled};` plus the preset constants.

### 4.3 Markdown component rewrite

**Scope:** only `markdown_to_lines` and its helpers in `crates/pi-tui/src/components/markdown.rs`. The `Markdown` struct's public surface (`new` / `with_padding` / `set_text` / `render`) is unchanged.

**Style mapping (standard set):**

| Markdown element | Current behavior | New behavior (color enabled) |
|---|---|---|
| `# Heading` | plain text, no visual distinction | `paint(text, bold())` |
| `` `code` `` inline | plain text | `paint(text, reverse())` |
| fenced code block | plain text, no fence | each content line `paint("   {line}", dim())`; fence rows `paint("\`\`\`", dim())` before and after |
| `> quote` | `> ` prefix plain text | `paint("> ...", dim())` whole line |
| `- item` / `1. item` | `- ` prefix | unchanged prefix; inline code within items still reverse |
| paragraph / plain text | plain text | plain text (default foreground) |
| `---` rule | `-` x 20 | unchanged; `paint(line, dim())` |
| soft break / hard break | merge / split | unchanged |

When color is disabled, `paint` returns plain text. Code-block fence rows still appear (they are literal text, not SGR-only), but without the dim wrapper. Inline code degrades to plain text (no reverse).

**Implementation notes:**

1. `pulldown-cmark`'s `Event::Text` / `Event::Code` remain the text accumulation entry points, but now inline-code spans must be tracked to wrap them with `reverse`. Approach: track `inline_code_spans: Vec<(usize, usize)>` (byte ranges in `current`) while accumulating; on `flush_current`, split the line by these spans and `paint` each code span with `reverse`.
2. Code blocks: while `in_code_block` is true, accumulated text is not subject to inline wrapping or word-wrap. Instead each source line is emitted as `paint("   {line}", dim())`, preceded and followed by a `paint("\`\`\`", dim())` fence row.
3. Headings: set a `heading: bool` flag on `Event::Start(Tag::Heading)`; on `flush_current` while `heading`, wrap the whole block with `paint(text, bold())`.
4. Block quotes: set `in_quote: bool` on `Event::Start(Tag::BlockQuote)`; on `flush_current` while `in_quote`, wrap the whole line with `paint(text, dim())`.

**Test impact:** existing `crates/pi-tui/tests/markdown.rs` assertions are plain-text. Updated assertions use `paint_with(..., true)` to assert exact ANSI bytes, and `paint_with(..., false)` to assert the degraded plain-text form. No reliance on `color_enabled()` in tests.

### 4.4 Transcript integration

**Scope:** `crates/pi-coding-agent/src/interactive/app.rs` — `render_transcript_lines`, `render_tool_lines`, `footer`, welcome, and the "new output below" indicator.

**4.4.1 `render_transcript_lines` (app.rs:893) per-role coloring:**

| TranscriptItem | Current | New |
|---|---|---|
| `User { text }` | `fit_line("user: {text}", w)` | `fit_line(&format!("{}: {}", paint("user", &USER), text), w)` — only the `user:` label is cyan; the prompt text stays default |
| `System { text }` | `fit_line(text, w)` | `fit_line(&paint(text, &SYSTEM), w)` — whole line dim |
| `Assistant { markdown }` | `Markdown::new(md).render()` | unchanged (styling lives inside Markdown now) |
| `Tool { ... }` | plain text | see 4.4.2 |
| `Error { text }` | `fit_line("error: {text}", w)` | `fit_line(&format!("{}: {}", paint("error", &ERROR), paint(text, &ERROR)), w)` — whole line red bold |

**4.4.2 `render_tool_lines` (app.rs:931) coloring:**

Current header: `format!("tool {name} {call_id} {status}")`.

New header:

    let status_style = match status {
        "running" => STATUS_RUNNING,
        "error"   => TOOL_ERROR,
        "done"    => STATUS_IDLE,  // dim, not loud
        _         => Style::default(),
    };
    let header = format!(
        "{} {} {} {}",
        paint("tool", &TOOL_NAME),
        paint(name, &TOOL_NAME),
        call_id,                          // call_id stays default for copyability
        paint(status, &status_style),
    );

Tool **result lines** (`result_lines`):

- `is_error == true`: each line `paint(line, &TOOL_ERROR)`.
- `is_error == false`: plain text (default foreground) — avoid drowning normal tool output in color.
- `"... truncated {N} lines"`: `paint(..., &SYSTEM)` dim.

**4.4.3 `footer` (app.rs:274) coloring:**

Only the `status` label+value and `cwd` value are colored; other parts stay default for readability.

    let status_style = match self.status {
        InteractiveStatus::Idle    => STATUS_IDLE,
        InteractiveStatus::Running => STATUS_RUNNING,
    };
    parts[0] = format!("{}: {}", paint("status", &status_style), paint(status_str, &status_style));
    parts[1] = format!("{}: {}", "cwd", paint(&cwd, &PATH));
    if self.usage != (0, 0) {
        parts.push(paint(&format!("↑{} ↓{}", ...), &SYSTEM));
    }

**4.4.4 Welcome line:** `TranscriptItem::System` already covers it — the `System` branch in `render_transcript_lines` dims the whole line, so the welcome inherits dim styling automatically. No special-casing.

**4.4.5 "... new output below" indicator (app.rs:987):** `paint(..., &SYSTEM)` dim.

**Invariant checks:**

- Every colored fragment goes through `paint`; when color is disabled, output degrades to plain text. Existing footer/test substring assertions like `assert!(frame.contains("status: idle"))` still hold — `paint` wraps with SGR but does not change visible characters.
- `visible_width` skips ANSI, so `fit_line` truncation is safe on colored lines.
- `scripted_interactive_footer_shows_usage_after_a_turn` asserts `frame.contains("↑")` — still holds.

### 4.5 Error handling

This slice introduces no new error types. `Style` / `Color` / `paint` are value types and string concatenation — no `Result`. The `Markdown` rewrite does not change the `render` signature (still returns `Vec<String>`). Transcript render function signatures are unchanged. `paint` returning plain text when color is disabled is not an error path.

The one boundary: if `pulldown-cmark` yields an unexpected AST shape, the existing `_ => {}` fallback arm in `markdown_to_lines` remains, ignoring unknown events — same as current behavior, no new panic risk.

### 4.6 Testing strategy

All tests are offline and deterministic.

**pi-tui layer:**

- `crates/pi-tui/tests/style.rs` (new):
  - `paint_with("hi", &Style::fg(Color::Red).bold(), true)` == `"\x1b[1;31mhi\x1b[0m"`.
  - `paint_with("hi", &Style::fg(Color::Red).bold(), false)` == `"hi"`.
  - Multiple modifiers merge: bold + reverse + red -> `\x1b[1;7;31m...\x1b[0m`.
  - `Color::Default` fg emits no fg sequence.
  - `Style::default()` (all default) with enabled=true returns plain text (nothing to wrap).
- `crates/pi-tui/tests/markdown.rs` (update existing + new):
  - With `paint_with(..., true)`: heading -> `\x1b[1m...\x1b[0m`; inline code -> `\x1b[7m...\x1b[0m`; code fence row -> contains `\x1b[2m`.
  - With `paint_with(..., false)`: degraded plain text (fence rows still present, no `\x1b[2m`).
  - Width safety: ANSI-bearing lines still measure correctly under `visible_width`.

**pi-coding-agent layer:**

- `crates/pi-coding-agent/src/interactive/app.rs` existing `#[cfg(test)] mod tests`:
  - Update `render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output`: assert tool header contains the yellow SGR for `tool`/`name` and the dim SGR for the truncated-notice line. Because these are in-file unit tests, they construct the transcript directly and can assert bytes without depending on `color_enabled()`.
  - New: error item rendering contains the red-bold SGR (`\x1b[1;31m`).
- `crates/pi-coding-agent/tests/interactive_mode.rs`:
  - Existing substring assertions (`status: idle`, `↑`, `↓`, `> typed`, `pi · `) stay unchanged — `paint` does not alter visible characters.
  - New ANSI-level assertions are kept in the in-file unit tests (which use `paint_with` explicitly), not in the scripted harness, to avoid TTY-dependent `color_enabled` caching.

**Risk: scripted tests and `color_enabled` cache TTY dependency.** Mitigation: scripted tests only assert text substrings (same as now). ANSI assertions live in `app.rs` in-file `#[cfg(test)]` unit tests, where `render_transcript_lines` output is constructed directly and bytes are checked. These unit tests do not depend on `color_enabled` (they use `paint_with` to control the flag explicitly).

### 4.7 File structure

| File | Operation |
|---|---|
| `pi-tui/src/style.rs` | new |
| `pi-tui/src/lib.rs` | edit: add `pub mod style;` + re-exports |
| `pi-tui/src/components/markdown.rs` | edit: rewrite `markdown_to_lines` + helpers |
| `pi-tui/tests/style.rs` | new |
| `pi-tui/tests/markdown.rs` | edit: update assertions |
| `pi-coding-agent/src/interactive/app.rs` | edit: color `render_transcript_lines`/`render_tool_lines`/`footer`/new-output indicator; update in-file tests |
| `pi-coding-agent/tests/interactive_mode.rs` | edit: keep substring assertions, no new ANSI assertions in scripted tests |

No new dependencies (`pulldown-cmark` / `unicode-segmentation` already present).

### 4.8 Verification

Run from `pi-rust/`:

    cargo fmt --check
    cargo test -p pi-tui
    cargo test -p pi-coding-agent
    cargo test --workspace
    cargo check --workspace

All must pass.

## 5. Key decisions and constraints

- **8-color semantic mapping**, not 256/true-color or themes — cross-terminal consistency and simplicity for this slice.
- **NO_COLOR + TERM=dumb only** for disabling — follows industry convention; no CLI flag or settings toggle this slice.
- **Style struct + paint function**, not builder pattern or raw constants — composable, testable, and the `paint` entry point centralizes the disable check.
- **`paint_with` for tests** — avoids dependence on the process-wide `OnceLock` cache that cannot be reset between tests.
- **Styles live inside components** — `Markdown` owns its styling; the transcript layer applies role colors to non-assistant rows. Matches the "component returns string, framework owns diff output" philosophy.
- **Inline code uses reverse**, not background fill — width-safe with existing `visible_width`/`fit_line`, no padding interaction.
- **Self-closing ANSI spans** — every `paint` result ends with `\x1b[0m`; `LINE_RESET` at line ends still guards against cross-line leaks.
- **Transcript substring assertions unchanged** — `paint` wraps but does not change visible characters, so existing `frame.contains(...)` tests stay green.
