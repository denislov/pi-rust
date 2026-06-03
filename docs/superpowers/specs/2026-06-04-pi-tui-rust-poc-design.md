# Design: Rust port of `pi-tui` core renderer (proof-of-concept)

- Date: 2026-06-04
- Status: Draft (pending review)
- Scope: First Rust PoC for the terminal UI crate.
- Depends on: none of the existing Rust crates.

## 1. Context

The TypeScript `@earendil-works/pi-tui` package is a terminal UI framework with:

- a simple component API (`render(width) -> string[]`);
- a terminal abstraction;
- differential rendering with synchronized output;
- width-aware text utilities;
- built-in components such as Text, Input, Editor, Markdown, SelectList, Image, and overlays.

The Rust workspace already contains an empty `crates/pi-tui` crate. This design covers the
first PoC only: the rendering foundation that future components can build on. It does not
attempt to port the full TS package in one phase.

## 2. Goals & success criteria

Build a `pi-tui` Rust crate that proves the same core design philosophy as the TS version:
components render width-bounded lines, the TUI owns terminal output, and repeated renders update
only the changed region when possible.

The PoC is done when:

1. `cargo test -p pi-tui` passes offline.
2. `cargo build -p pi-tui` and the full workspace build cleanly.
3. The crate exposes:
   - `Component` trait.
   - `Container` component.
   - `Tui<T: Terminal>` render manager.
   - `Terminal` trait.
   - in-memory `VirtualTerminal` test backend.
   - `ProcessTerminal` backend for real terminal output.
   - `Text` and `Spacer` components.
   - `visible_width()`, `truncate_to_width()`, and ANSI-aware width helpers.
4. Tests cover:
   - visible width for ASCII, CJK, emoji, tabs, and ANSI sequences;
   - truncation by terminal columns;
   - Text wrapping and width guarantees;
   - Container child ordering;
   - first render full redraw;
   - second render differential update from the first changed line;
   - content shrink clearing behavior;
   - line-width violation returning an error instead of corrupting terminal state.

Non-goal: `ratatui`, Editor, Input, key parsing, stdin buffering, bracketed paste, overlays,
Markdown, inline images, Kitty keyboard protocol, IME cursor marker support, async render loop,
theme system, and session-level app wiring.

## 3. Key decisions

- **Do not use `ratatui` for the PoC.** `ratatui` is mature, but its `Frame`/`Buffer`/`Widget`
  model would pull the Rust port away from the TS `pi-tui` philosophy. This crate should remain
  a direct port of the pi rendering model: components return strings and the framework owns
  differential terminal output.
- **Use mature low-level crates.**
  - `crossterm` for real terminal commands and terminal size.
  - `unicode-width` for display-cell width.
  - `unicode-segmentation` for grapheme-safe truncation.
  - `thiserror` for typed errors.
- **Keep rendering synchronous in the PoC.** `Tui::render_once()` is explicit and testable. A
  future phase can add throttled `request_render()` and an input loop.
- **Prefer a generic terminal backend:** `Tui<T: Terminal>` owns a terminal implementation. Tests
  use `VirtualTerminal`; real programs use `ProcessTerminal`.
- **Fail fast on invalid component output.** If a component returns a line wider than the
  terminal width, `render_once()` returns `TuiError::LineTooWide` before writing a partial frame.
- **Use synchronized output boundaries.** Each render writes CSI 2026 enable/disable sequences
  around terminal updates, matching the TS package's flicker-reduction strategy.
- **Start with deterministic tests.** The PoC does not require a real TTY. `VirtualTerminal`
  records writes and maintains a simple viewport model that is sufficient for renderer tests.

## 4. Scope

### In scope

- Crate layout and public re-exports.
- Width utilities:
  - ANSI/OSC/APC escape skipping for width measurement.
  - tab width as 3 columns, matching TS behavior.
  - grapheme-safe truncation.
- Components:
  - `Component` trait.
  - `Container`.
  - `Text`.
  - `Spacer`.
- Rendering:
  - first render full redraw;
  - full redraw on width/height change;
  - optional full redraw when content shrinks;
  - normal update from first changed line;
  - synchronized output wrapper;
  - full reset suffix per rendered line.
- Terminal abstraction:
  - `TerminalSize`.
  - `Terminal` trait.
  - `VirtualTerminal`.
  - `ProcessTerminal` backed by `crossterm` and `stdout`.
- Offline tests.
- Optional example that renders a static screen once.

### Out of scope

- `ratatui`.
- Raw mode and stdin input.
- Key parsing and keybindings.
- `Input`, `Editor`, autocomplete, kill ring, undo stack, word navigation.
- Markdown rendering.
- Overlay stack and focus management.
- Inline images and terminal image capability detection.
- Hardware cursor and IME cursor marker.
- Async timers, throttled render requests, and animation loops.

## 5. Architecture

### 5.1 Crate layout

```text
crates/pi-tui/
  Cargo.toml
  src/
    lib.rs
    component.rs       # Component trait, Container
    terminal.rs        # Terminal trait, TerminalSize, ProcessTerminal
    tui.rs             # Tui<T>, render algorithm, TuiError
    virtual_terminal.rs# in-memory terminal backend for tests and examples
    utils/
      mod.rs
      ansi.rs          # ANSI/OSC/APC escape parsing helpers
      width.rs         # visible_width, truncate_to_width
    components/
      mod.rs
      text.rs
      spacer.rs
  tests/
    width.rs
    components.rs
    tui_render.rs
  examples/
    render_once.rs
```

### 5.2 Public API shape

```rust
pub trait Component {
    fn render(&mut self, width: usize) -> Vec<String>;
    fn invalidate(&mut self) {}
}

pub struct Container {
    children: Vec<Box<dyn Component>>,
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
```

`Tui::render_once()` returns `Result<RenderOutcome, TuiError>`, where `RenderOutcome` records
whether a full redraw was used and the first changed line for differential renders.

### 5.3 Render algorithm

1. Read terminal size.
2. Render all children with the current width and concatenate their lines.
3. Validate every line with `visible_width(line) <= width`.
4. Decide render strategy:
   - full redraw if no previous frame exists;
   - full redraw if width changed;
   - full redraw if height changed;
   - full redraw if content shrank and `clear_on_shrink` is enabled;
   - otherwise differential redraw from the first changed line.
5. Wrap all writes in:
   - start: `\x1b[?2026h`
   - end: `\x1b[?2026l`
6. For a full redraw:
   - hide cursor;
   - clear screen;
   - write all lines from row 0.
7. For a differential redraw:
   - move from the current logical cursor row to the first changed row;
   - clear from cursor to end;
   - write changed lines through the end of the new frame.
8. Append `\x1b[0m\x1b]8;;\x07` to every rendered line to reset SGR and OSC 8 hyperlink state.
9. Update `previous_lines`, dimensions, and cursor row only after all terminal writes succeed.

### 5.4 Width utilities

`visible_width()`:

- counts printable ASCII directly;
- treats tab as width 3;
- skips ANSI CSI sequences, OSC sequences, and APC sequences;
- uses `unicode-segmentation` to iterate grapheme clusters;
- uses `unicode-width` to compute grapheme display width.

`truncate_to_width()`:

- never splits a grapheme cluster;
- keeps ANSI escape sequences that occur before retained visible text;
- returns a string whose `visible_width()` is at most the requested width;
- supports optional padding in a future phase, but PoC only returns the clipped string.

### 5.5 Components

`Text`:

- stores a string.
- `render(width)` wraps words on whitespace.
- very long words are truncated/sliced by terminal width.
- every returned line satisfies `visible_width(line) <= width`.

`Spacer`:

- stores a height.
- `render(width)` returns that many empty strings.

`Container`:

- owns child components.
- renders children in insertion order and concatenates their lines.

### 5.6 Terminal backends

`VirtualTerminal`:

- stores size, write log, operation log, and a simple viewport.
- supports resize in tests.
- is intentionally not a full VT emulator; renderer tests assert high-level behavior and write
  order rather than every terminal cursor edge case.

`ProcessTerminal`:

- writes to stdout.
- uses `crossterm::terminal::size()` for dimensions.
- emits ANSI/crossterm commands for cursor movement and clearing.
- does not manage raw mode or input in this PoC.

## 6. Error handling

```rust
pub enum TuiError {
    Io(std::io::Error),
    LineTooWide {
        line_index: usize,
        width: usize,
        max_width: usize,
        line: String,
    },
}
```

All terminal I/O errors are returned to the caller. A line-width violation is detected before
any terminal writes are performed for that frame.

## 7. Testing strategy

All tests run offline and do not require a real terminal.

- `tests/width.rs`
  - ASCII width.
  - CJK width.
  - emoji width.
  - tab width.
  - ANSI/OSC/APC sequences are zero-width.
  - truncation does not split CJK/emoji graphemes.
- `tests/components.rs`
  - `Text` wraps simple text to width.
  - `Text` handles long words without exceeding width.
  - `Spacer` returns empty lines.
  - `Container` preserves child order.
- `tests/tui_render.rs`
  - first render uses synchronized output and full redraw.
  - second render with one changed middle line does not clear the full screen.
  - width change triggers full redraw.
  - shrink with `clear_on_shrink = true` triggers full redraw.
  - line too wide returns `TuiError::LineTooWide` and leaves write log unchanged.

## 8. Dependencies

```toml
[dependencies]
crossterm = "0.28"
thiserror = "2"
unicode-segmentation = "1"
unicode-width = "0.2"
```

No `ratatui` dependency in this PoC.

## 9. Risks

- **VirtualTerminal is simpler than a real terminal.** Mitigation: keep tests focused on renderer
  decisions and write ordering; add a VT parser or snapshot tests in a later phase if needed.
- **Unicode width differs across terminals.** Mitigation: use standard crates now, then port
  TS-specific edge cases such as regional indicators and Thai/Lao AM vowels in a later phase.
- **Differential rendering can drift if cursor accounting is wrong.** Mitigation: keep the PoC
  algorithm small and cover first render, middle-line updates, shrink, and resize.
- **Too much framework surface too early.** Mitigation: no input, overlays, images, markdown, or
  editor in this phase.

## 10. Future phases

- Input stack: raw mode, stdin buffering, bracketed paste, key parsing, keybindings.
- Focus and cursor positioning.
- `Input` and `Editor`.
- Overlay stack and focus restoration.
- Markdown component via `pulldown-cmark`.
- Inline images and Kitty/iTerm2 protocols.
- More exact Unicode behavior matching the TS utilities.
- Optional async render scheduling and `request_render()`.
