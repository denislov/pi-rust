# M6 Interactive TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Build the Rust M6 interactive TUI vertical slice: terminal input, focus/overlay/runtime support, core interactive components, and a default `pi-coding-agent` TUI mode that can submit prompts and render Markdown/tool output.

**Architecture:** Keep reusable terminal primitives in `pi-tui` and coding-agent-specific transcript/runtime wiring in `pi-coding-agent`. Preserve the existing string-rendering `Component` model and `VirtualTerminal` tests while adding raw input parsing, async render coalescing, focus, overlays, cursor marker support, and a UI event bridge around the existing session-backed prompt runner.

**Tech Stack:** Rust edition 2024, existing `crossterm`, `unicode-width`, `unicode-segmentation`, `thiserror`, existing `tokio` in `pi-coding-agent`, new `pulldown-cmark` for Markdown parsing, existing faux provider and temp-dir based tests.

**Spec:** `docs/superpowers/specs/2026-06-11-pi-tui-m6-interactive-tui-design.md`

---

## File Structure

- Modify `crates/pi-tui/Cargo.toml` - add `pulldown-cmark` and async/lifecycle dependencies only if required by implementation.
- Modify `crates/pi-tui/src/lib.rs` - export input, runtime, overlay, cursor, and new components.
- Modify `crates/pi-tui/src/component.rs` - add optional input hooks and focus helper support while preserving current render behavior.
- Modify `crates/pi-tui/src/terminal.rs` - add terminal lifecycle controls, title/progress operations, and optional input start/stop support.
- Modify `crates/pi-tui/src/tui.rs` - add focus, input dispatch, cursor marker extraction, overlay composition, and forced render support.
- Modify `crates/pi-tui/src/virtual_terminal.rs` - record lifecycle/title/progress/cursor operations for deterministic tests.
- Create `crates/pi-tui/src/input/mod.rs` - public input exports.
- Create `crates/pi-tui/src/input/key.rs` - `Key`, `KeyEvent`, modifier parsing, Kitty CSI-u support, `parse_key`, `matches_key`.
- Create `crates/pi-tui/src/input/keybindings.rs` - keybinding definitions, manager, defaults, conflict detection.
- Create `crates/pi-tui/src/input/stdin_buffer.rs` - partial escape sequence buffering and bracketed paste parsing.
- Create `crates/pi-tui/src/runtime.rs` - render coalescing and input event pump around `Tui<T>`.
- Create `crates/pi-tui/src/overlay.rs` - overlay options, handle, placement helpers.
- Create `crates/pi-tui/src/cursor.rs` - cursor marker constant and extraction helper.
- Create `crates/pi-tui/src/kill_ring.rs` - kill/yank support.
- Create `crates/pi-tui/src/undo_stack.rs` - bounded undo stack.
- Create `crates/pi-tui/src/word_navigation.rs` - word-boundary helpers.
- Create `crates/pi-tui/src/components/input.rs` - single-line input component.
- Create `crates/pi-tui/src/components/editor.rs` - multi-line prompt editor component.
- Create `crates/pi-tui/src/components/select_list.rs` - scrollable selection list.
- Create `crates/pi-tui/src/components/markdown.rs` - Markdown renderer.
- Optionally create `crates/pi-tui/src/components/truncated_text.rs` and `crates/pi-tui/src/components/box.rs` if the transcript/overlay implementation needs them.
- Create `crates/pi-tui/tests/input_stack.rs`.
- Create `crates/pi-tui/tests/keybindings.rs`.
- Create `crates/pi-tui/tests/tui_runtime.rs`.
- Create `crates/pi-tui/tests/overlay.rs`.
- Create `crates/pi-tui/tests/cursor.rs`.
- Create `crates/pi-tui/tests/input_component.rs`.
- Create `crates/pi-tui/tests/editor_component.rs`.
- Create `crates/pi-tui/tests/select_list.rs`.
- Create `crates/pi-tui/tests/markdown.rs`.
- Modify `crates/pi-coding-agent/Cargo.toml` - add dependency on `pi-tui`.
- Modify `crates/pi-coding-agent/src/args.rs` - keep default mode route available for interactive startup.
- Modify `crates/pi-coding-agent/src/lib.rs` - route default non-print, non-explicit-mode invocations to interactive mode.
- Create `crates/pi-coding-agent/src/interactive/mod.rs`.
- Create `crates/pi-coding-agent/src/interactive/app.rs`.
- Create `crates/pi-coding-agent/src/interactive/event_bridge.rs`.
- Create `crates/pi-coding-agent/src/interactive/transcript.rs`.
- Create `crates/pi-coding-agent/src/interactive/components/mod.rs`.
- Create `crates/pi-coding-agent/src/interactive/components/user_message.rs`.
- Create `crates/pi-coding-agent/src/interactive/components/assistant_message.rs`.
- Create `crates/pi-coding-agent/src/interactive/components/tool_execution.rs`.
- Create `crates/pi-coding-agent/src/interactive/components/footer.rs`.
- Create `crates/pi-coding-agent/tests/interactive_args.rs`.
- Create `crates/pi-coding-agent/tests/interactive_event_bridge.rs`.
- Create `crates/pi-coding-agent/tests/interactive_transcript.rs`.
- Create `crates/pi-coding-agent/tests/interactive_mode.rs`.
- Create `crates/pi-coding-agent/tests/interactive_abort.rs`.
- Create `crates/pi-coding-agent/tests/interactive_sessions.rs`.

## Task 1: Input stack and keybindings

**Files:**
- Create: `crates/pi-tui/src/input/mod.rs`
- Create: `crates/pi-tui/src/input/key.rs`
- Create: `crates/pi-tui/src/input/keybindings.rs`
- Create: `crates/pi-tui/src/input/stdin_buffer.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/input_stack.rs`
- Test: `crates/pi-tui/tests/keybindings.rs`

- [ ] **Step 1: Write failing input parsing tests**

Create `crates/pi-tui/tests/input_stack.rs`:

```rust
use pi_tui::{
    InputEvent, Key, KeyEventKind, KeyModifiers, StdinBuffer, matches_key, parse_key,
};

#[test]
fn stdin_buffer_splits_batched_escape_sequences() {
    let mut buffer = StdinBuffer::new();
    let events = buffer.process("\x1b[A\x1b[Bx");
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], InputEvent::Key(_)));
    assert!(matches!(events[1], InputEvent::Key(_)));
    assert!(matches!(events[2], InputEvent::Key(_)));
    assert!(matches_key(&events[0], "up"));
    assert!(matches_key(&events[1], "down"));
    assert!(matches_key(&events[2], "x"));
}

#[test]
fn stdin_buffer_waits_for_partial_csi_sequence() {
    let mut buffer = StdinBuffer::new();
    assert!(buffer.process("\x1b[").is_empty());
    let events = buffer.process("A");
    assert_eq!(events.len(), 1);
    assert!(matches_key(&events[0], "up"));
}

#[test]
fn bracketed_paste_is_one_paste_event() {
    let mut buffer = StdinBuffer::new();
    let events = buffer.process("\x1b[200~hello\nworld\x1b[201~");
    assert_eq!(events, vec![InputEvent::Paste("hello\nworld".to_string())]);
}

#[test]
fn parse_legacy_and_kitty_keys() {
    assert!(matches_key(&InputEvent::Key(parse_key("\r").unwrap()), "enter"));
    assert!(matches_key(&InputEvent::Key(parse_key("\x7f").unwrap()), "backspace"));
    assert!(matches_key(&InputEvent::Key(parse_key("\x1b[3~").unwrap()), "delete"));
    assert!(matches_key(&InputEvent::Key(parse_key("\x1b[97u").unwrap()), "a"));
    assert!(matches_key(&InputEvent::Key(parse_key("\x1b[65;5u").unwrap()), "ctrl+shift+a"));
}

#[test]
fn kitty_release_events_are_detected() {
    let event = parse_key("\x1b[97;3:3u").unwrap();
    assert_eq!(event.key, Key::Char("a".to_string()));
    assert_eq!(event.kind, KeyEventKind::Release);
    assert_eq!(event.modifiers, KeyModifiers::SHIFT);
}
```

- [ ] **Step 2: Write failing keybinding tests**

Create `crates/pi-tui/tests/keybindings.rs`:

```rust
use pi_tui::{InputEvent, KeybindingConflict, KeybindingsManager, TUI_KEYBINDINGS, parse_key};

fn key(input: &str) -> InputEvent {
    InputEvent::Key(parse_key(input).unwrap())
}

#[test]
fn default_keybindings_match_editor_actions() {
    let manager = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    assert!(manager.matches(&key("\x1b[A"), "tui.editor.cursorUp"));
    assert!(manager.matches(&key("\x1b[B"), "tui.editor.cursorDown"));
    assert!(manager.matches(&key("\r"), "tui.input.submit"));
}

#[test]
fn user_bindings_override_defaults() {
    let mut user = std::collections::BTreeMap::new();
    user.insert("tui.input.submit".to_string(), vec!["ctrl+j".to_string()]);
    let manager = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), user);
    assert!(!manager.matches(&key("\r"), "tui.input.submit"));
    assert!(manager.matches(&key("\n"), "tui.input.submit"));
}

#[test]
fn conflicts_are_reported_for_user_bindings() {
    let mut user = std::collections::BTreeMap::new();
    user.insert("tui.input.submit".to_string(), vec!["ctrl+x".to_string()]);
    user.insert("tui.select.cancel".to_string(), vec!["ctrl+x".to_string()]);
    let manager = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), user);
    assert_eq!(
        manager.conflicts(),
        vec![KeybindingConflict {
            key: "ctrl+x".to_string(),
            keybindings: vec![
                "tui.input.submit".to_string(),
                "tui.select.cancel".to_string(),
            ],
        }]
    );
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-tui --test input_stack
cargo test -p pi-tui --test keybindings
```

Expected: both fail to compile because the input stack does not exist.

- [ ] **Step 4: Implement input event types and parser**

In `crates/pi-tui/src/input/key.rs`, define:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(String),
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Function(u8),
    Unknown(String),
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0001;
        const ALT = 0b0010;
        const CTRL = 0b0100;
        const SUPER = 0b1000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
}
```

Add `bitflags = "2"` to `crates/pi-tui/Cargo.toml` if using the snippet above.

Implement:

```rust
pub fn parse_key(data: &str) -> Option<KeyEvent>;
pub fn matches_key(event: &InputEvent, key_id: &str) -> bool;
pub fn is_key_release(event: &InputEvent) -> bool;
```

The parser must cover the exact inputs asserted in `input_stack.rs` and leave unknown escape
sequences as `Key::Unknown(data.to_string())` instead of panicking.

- [ ] **Step 5: Implement stdin buffering**

In `crates/pi-tui/src/input/stdin_buffer.rs`, add:

```rust
#[derive(Debug, Clone)]
pub struct StdinBuffer {
    buffer: String,
    paste_buffer: String,
    in_paste: bool,
}

impl StdinBuffer {
    pub fn new() -> Self;
    pub fn process(&mut self, data: &str) -> Vec<InputEvent>;
    pub fn flush(&mut self) -> Vec<InputEvent>;
}
```

Match TypeScript framing rules:

- CSI complete when final byte is in `0x40..=0x7e`.
- OSC complete on BEL or `ESC \`.
- DCS complete on `ESC \`.
- APC complete on `ESC \`.
- SS3 complete after `ESC O` plus one byte.
- bracketed paste start is `\x1b[200~`; end is `\x1b[201~`.

- [ ] **Step 6: Implement keybindings**

In `crates/pi-tui/src/input/keybindings.rs`, define:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingDefinition {
    pub default_keys: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindingConflict {
    pub key: String,
    pub keybindings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct KeybindingsManager {
    definitions: std::collections::BTreeMap<String, KeybindingDefinition>,
    keys_by_id: std::collections::BTreeMap<String, Vec<String>>,
    conflicts: Vec<KeybindingConflict>,
}
```

Export `TUI_KEYBINDINGS` with the TypeScript default action ids from
`pi/packages/tui/src/keybindings.ts`, including editor, input, and select-list actions.

- [ ] **Step 7: Export public symbols**

Update `crates/pi-tui/src/input/mod.rs`:

```rust
mod key;
mod keybindings;
mod stdin_buffer;

pub use key::{Key, KeyEvent, KeyEventKind, KeyModifiers, is_key_release, matches_key, parse_key};
pub use keybindings::{
    KeybindingConflict, KeybindingDefinition, KeybindingsConfig, KeybindingsManager,
    TUI_KEYBINDINGS,
};
pub use stdin_buffer::StdinBuffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    Raw(String),
    Resize(crate::TerminalSize),
}
```

Update `crates/pi-tui/src/lib.rs` to `pub mod input;` and re-export the public input symbols.

- [ ] **Step 8: Run input tests**

Run:

```bash
cargo test -p pi-tui --test input_stack
cargo test -p pi-tui --test keybindings
```

Expected: both pass.

## Task 2: Terminal lifecycle and virtual terminal coverage

**Files:**
- Modify: `crates/pi-tui/src/terminal.rs`
- Modify: `crates/pi-tui/src/virtual_terminal.rs`
- Test: `crates/pi-tui/tests/terminal_lifecycle.rs`

- [ ] **Step 1: Write failing terminal lifecycle tests**

Create `crates/pi-tui/tests/terminal_lifecycle.rs`:

```rust
use pi_tui::{Terminal, TerminalOp, VirtualTerminal};

#[test]
fn virtual_terminal_records_lifecycle_operations() {
    let mut terminal = VirtualTerminal::new(80, 24);
    terminal.start().unwrap();
    terminal.set_title("pi").unwrap();
    terminal.set_progress(true).unwrap();
    terminal.set_progress(false).unwrap();
    terminal.stop().unwrap();

    assert_eq!(
        terminal.ops(),
        &[
            TerminalOp::Start,
            TerminalOp::SetTitle("pi".to_string()),
            TerminalOp::SetProgress(true),
            TerminalOp::SetProgress(false),
            TerminalOp::Stop,
        ]
    );
}

#[test]
fn virtual_terminal_reports_kitty_protocol_state() {
    let mut terminal = VirtualTerminal::new(80, 24);
    assert!(!terminal.kitty_protocol_active());
    terminal.set_kitty_protocol_active(true);
    assert!(terminal.kitty_protocol_active());
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test -p pi-tui --test terminal_lifecycle
```

Expected: FAIL because lifecycle operations are not part of `Terminal`.

- [ ] **Step 3: Extend the terminal trait**

Update `crates/pi-tui/src/terminal.rs`:

```rust
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

    fn start(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn stop(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn drain_input(&mut self, _max: std::time::Duration, _idle: std::time::Duration) -> std::io::Result<()> {
        Ok(())
    }

    fn set_title(&mut self, _title: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn set_progress(&mut self, _active: bool) -> std::io::Result<()> {
        Ok(())
    }

    fn kitty_protocol_active(&self) -> bool {
        false
    }
}
```

- [ ] **Step 4: Implement `ProcessTerminal` lifecycle**

In `ProcessTerminal::start()`:

- save whether stdin was already raw when that is available through `crossterm`;
- enable raw mode with `crossterm::terminal::enable_raw_mode()`;
- write bracketed paste enable `\x1b[?2004h`;
- write Kitty keyboard protocol query `\x1b[>7u\x1b[?u\x1b[c`;
- hide cursor.

In `ProcessTerminal::stop()`:

- write bracketed paste disable `\x1b[?2004l`;
- write Kitty keyboard protocol pop/disable sequence `\x1b[<u`;
- show cursor;
- disable raw mode only if M6 enabled it.

`set_title("x")` writes `\x1b]0;x\x07`. `set_progress(true)` writes `\x1b]9;4;3\x07`;
`set_progress(false)` writes `\x1b]9;4;0;\x07`.

- [ ] **Step 5: Extend `VirtualTerminal`**

Add terminal operations:

```rust
Start,
Stop,
DrainInput { max_ms: u64, idle_ms: u64 },
SetTitle(String),
SetProgress(bool),
SetKittyProtocolActive(bool),
```

Add:

```rust
pub fn set_kitty_protocol_active(&mut self, active: bool);
```

- [ ] **Step 6: Run terminal lifecycle tests**

Run:

```bash
cargo test -p pi-tui --test terminal_lifecycle
```

Expected: PASS.

## Task 3: TUI runtime, focus, and cursor marker

**Files:**
- Modify: `crates/pi-tui/src/component.rs`
- Modify: `crates/pi-tui/src/tui.rs`
- Create: `crates/pi-tui/src/cursor.rs`
- Create: `crates/pi-tui/src/runtime.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/cursor.rs`
- Test: `crates/pi-tui/tests/tui_runtime.rs`

- [ ] **Step 1: Write failing cursor marker tests**

Create `crates/pi-tui/tests/cursor.rs`:

```rust
use pi_tui::{CURSOR_MARKER, extract_cursor_marker};

#[test]
fn cursor_marker_is_stripped_and_column_is_visible_width() {
    let mut lines = vec![
        "before".to_string(),
        format!("a\x1b[31m好\x1b[0m{CURSOR_MARKER}z"),
    ];

    let cursor = extract_cursor_marker(&mut lines, 24).unwrap();
    assert_eq!(cursor.row, 1);
    assert_eq!(cursor.col, 3);
    assert_eq!(lines[1], "a\x1b[31m好\x1b[0mz");
}

#[test]
fn cursor_marker_only_scans_visible_viewport() {
    let mut lines = vec![
        format!("old{CURSOR_MARKER}"),
        "visible".to_string(),
    ];

    assert_eq!(extract_cursor_marker(&mut lines, 1), None);
    assert_eq!(lines[0], format!("old{CURSOR_MARKER}"));
}
```

- [ ] **Step 2: Write failing runtime focus tests**

Create `crates/pi-tui/tests/tui_runtime.rs`:

```rust
use pi_tui::{Component, InputEvent, Key, KeyEvent, KeyEventKind, KeyModifiers, Tui, VirtualTerminal};

#[derive(Default)]
struct RecordingComponent {
    focused: bool,
    inputs: Vec<InputEvent>,
}

impl Component for RecordingComponent {
    fn render(&mut self, _width: usize) -> Vec<String> {
        vec![if self.focused { "focused" } else { "idle" }.to_string()]
    }

    fn handle_input(&mut self, event: &InputEvent) {
        self.inputs.push(event.clone());
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn focused_component_receives_input() {
    let terminal = VirtualTerminal::new(20, 5);
    let mut tui = Tui::new(terminal);
    let id = tui.add_child_with_id(Box::new(RecordingComponent::default()));
    tui.set_focus(Some(id));
    tui.dispatch_input(&InputEvent::Key(KeyEvent {
        key: Key::Char("x".to_string()),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
    }));
    let component = tui.component_as::<RecordingComponent>(id).unwrap();
    assert_eq!(component.inputs.len(), 1);
    assert!(component.focused);
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-tui --test cursor
cargo test -p pi-tui --test tui_runtime
```

Expected: FAIL because cursor and focus APIs do not exist.

- [ ] **Step 4: Add cursor helper**

Create `crates/pi-tui/src/cursor.rs`:

```rust
pub const CURSOR_MARKER: &str = "\x1b_pi:c\x07";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: usize,
    pub col: usize,
}

pub fn extract_cursor_marker(lines: &mut [String], terminal_height: usize) -> Option<CursorPosition>;
```

Implementation scans only `lines.len().saturating_sub(terminal_height)..lines.len()`, uses
`visible_width()` on text before the marker, removes the marker, and returns row/column.

- [ ] **Step 5: Extend `Component` without breaking existing components**

Update `crates/pi-tui/src/component.rs`:

```rust
pub type ComponentId = usize;

pub trait Component {
    fn render(&mut self, width: usize) -> Vec<String>;

    fn handle_input(&mut self, _event: &crate::InputEvent) {}

    fn wants_key_release(&self) -> bool {
        false
    }

    fn set_focused(&mut self, _focused: bool) {}

    fn focused(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn std::any::Any;

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    fn invalidate(&mut self) {}
}
```

Add a small macro or manual implementations for existing built-in components so tests can downcast
through `component_as::<T>()`.

- [ ] **Step 6: Add focus and input dispatch to `Tui`**

Add to `Tui<T>`:

```rust
children: Vec<(ComponentId, Box<dyn Component>)>,
next_component_id: ComponentId,
focused_component: Option<ComponentId>,
input_listeners: Vec<Box<dyn FnMut(&InputEvent) -> InputListenerResult>>,
```

Add methods:

```rust
pub fn add_child_with_id(&mut self, child: Box<dyn Component>) -> ComponentId;
pub fn remove_child(&mut self, id: ComponentId) -> Option<Box<dyn Component>>;
pub fn set_focus(&mut self, id: Option<ComponentId>);
pub fn dispatch_input(&mut self, event: &InputEvent);
pub fn component_as<C: 'static>(&self, id: ComponentId) -> Option<&C>;
pub fn component_as_mut<C: 'static>(&mut self, id: ComponentId) -> Option<&mut C>;
```

Keep existing `add_child()` as a compatibility wrapper that discards the id.

- [ ] **Step 7: Add render request coalescing helper**

Create `crates/pi-tui/src/runtime.rs` with a testable scheduler:

```rust
pub struct RenderScheduler {
    requested: bool,
    force: bool,
    min_interval: std::time::Duration,
    last_render_at: Option<std::time::Instant>,
}

impl RenderScheduler {
    pub fn new(min_interval: std::time::Duration) -> Self;
    pub fn request(&mut self, force: bool);
    pub fn should_render_now(&self, now: std::time::Instant) -> bool;
    pub fn mark_rendered(&mut self, now: std::time::Instant) -> bool;
}
```

`TuiRuntime<T>` can wrap `Tui<T>` later; the scheduler tests should not require sleeping.

- [ ] **Step 8: Integrate cursor marker into `render_once()`**

In `Tui::render_once()`:

- render child lines;
- composite overlays later after Task 4;
- call `extract_cursor_marker(&mut lines, height)`;
- apply line resets;
- write lines;
- after writing, move the hardware cursor to the extracted row/column or hide it.

Keep existing tests green by hiding the cursor when no marker exists.

- [ ] **Step 9: Run runtime tests and existing pi-tui tests**

Run:

```bash
cargo test -p pi-tui --test cursor
cargo test -p pi-tui --test tui_runtime
cargo test -p pi-tui
```

Expected: all pass.

## Task 4: Overlay stack

**Files:**
- Create: `crates/pi-tui/src/overlay.rs`
- Modify: `crates/pi-tui/src/tui.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/overlay.rs`

- [ ] **Step 1: Write failing overlay tests**

Create `crates/pi-tui/tests/overlay.rs`:

```rust
use pi_tui::{Component, OverlayAnchor, OverlayOptions, Tui, VirtualTerminal};

struct Lines(Vec<String>);

impl Component for Lines {
    fn render(&mut self, _width: usize) -> Vec<String> {
        self.0.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn centered_overlay_is_composited_over_base_lines() {
    let mut tui = Tui::new(VirtualTerminal::new(10, 5));
    tui.add_child(Box::new(Lines(vec![
        "..........".to_string(),
        "..........".to_string(),
        "..........".to_string(),
    ])));
    tui.show_overlay(
        Box::new(Lines(vec!["XX".to_string()])),
        OverlayOptions {
            anchor: OverlayAnchor::Center,
            width: Some(2.into()),
            ..Default::default()
        },
    );
    tui.render_once().unwrap();
    let output = tui.terminal().written_output();
    assert!(output.contains("....XX...."));
}

#[test]
fn hiding_overlay_restores_base_render() {
    let mut tui = Tui::new(VirtualTerminal::new(8, 4));
    tui.add_child(Box::new(Lines(vec!["base".to_string()])));
    let handle = tui.show_overlay(Box::new(Lines(vec!["menu".to_string()])), Default::default());
    handle.hide(&mut tui);
    tui.render_once().unwrap();
    assert!(tui.terminal().written_output().contains("base"));
    assert!(!tui.terminal().written_output().contains("menu"));
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test -p pi-tui --test overlay
```

Expected: FAIL because overlay APIs do not exist.

- [ ] **Step 3: Implement overlay types**

Create `crates/pi-tui/src/overlay.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAnchor {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
    LeftCenter,
    RightCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeValue {
    Columns(usize),
    Percent(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlayMargin {
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
    pub left: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayOptions {
    pub width: Option<SizeValue>,
    pub min_width: Option<usize>,
    pub max_height: Option<SizeValue>,
    pub anchor: OverlayAnchor,
    pub offset_x: isize,
    pub offset_y: isize,
    pub row: Option<SizeValue>,
    pub col: Option<SizeValue>,
    pub margin: OverlayMargin,
    pub non_capturing: bool,
}
```

Implement `Default` with center anchor, no explicit width, zero margin.

- [ ] **Step 4: Add overlay stack to `Tui`**

Add `show_overlay()`, `hide_overlay()`, `has_overlay()`, and internal compositing. For M6, make
`OverlayHandle` an id-like value:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayHandle {
    id: usize,
}
```

Methods that mutate an overlay can take `&mut Tui<T>`:

```rust
impl OverlayHandle {
    pub fn hide<T: Terminal>(self, tui: &mut Tui<T>);
    pub fn set_hidden<T: Terminal>(self, tui: &mut Tui<T>, hidden: bool);
    pub fn focus<T: Terminal>(self, tui: &mut Tui<T>);
    pub fn unfocus<T: Terminal>(self, tui: &mut Tui<T>, target: Option<ComponentId>);
}
```

- [ ] **Step 5: Implement width-safe compositing**

Compositing rules:

- render base lines first;
- render visible overlays in focus order;
- resolve overlay width before rendering component;
- clamp row/column inside terminal area after margins;
- splice overlay lines into base lines using `slice_by_column`-style helpers from `utils`;
- pad overlay width with spaces so old content is covered;
- validate final lines through existing `LineTooWide` path.

- [ ] **Step 6: Run overlay tests**

Run:

```bash
cargo test -p pi-tui --test overlay
cargo test -p pi-tui
```

Expected: all pass.

## Task 5: Input and Editor components

**Files:**
- Create: `crates/pi-tui/src/kill_ring.rs`
- Create: `crates/pi-tui/src/undo_stack.rs`
- Create: `crates/pi-tui/src/word_navigation.rs`
- Create: `crates/pi-tui/src/components/input.rs`
- Create: `crates/pi-tui/src/components/editor.rs`
- Modify: `crates/pi-tui/src/components/mod.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/input_component.rs`
- Test: `crates/pi-tui/tests/editor_component.rs`

- [ ] **Step 1: Write failing single-line input tests**

Create `crates/pi-tui/tests/input_component.rs`:

```rust
use pi_tui::{Component, Input, InputEvent, KeybindingsManager, StdinBuffer, TUI_KEYBINDINGS};

fn feed(input: &mut Input, data: &str) {
    let mut buffer = StdinBuffer::new();
    for event in buffer.process(data) {
        input.handle_input(&event);
    }
}

#[test]
fn input_edits_unicode_graphemes() {
    let mut input = Input::new(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()));
    feed(&mut input, "a好");
    assert_eq!(input.value(), "a好");
    feed(&mut input, "\x7f");
    assert_eq!(input.value(), "a");
}

#[test]
fn input_paste_inserts_literal_content() {
    let mut input = Input::new(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()));
    feed(&mut input, "\x1b[200~hello\nworld\x1b[201~");
    assert_eq!(input.value(), "hello\nworld");
}

#[test]
fn focused_input_renders_cursor_marker() {
    let mut input = Input::new(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()));
    input.set_focused(true);
    feed(&mut input, "abc");
    let line = input.render(10).join("");
    assert!(line.contains(pi_tui::CURSOR_MARKER));
}
```

- [ ] **Step 2: Write failing editor tests**

Create `crates/pi-tui/tests/editor_component.rs`:

```rust
use pi_tui::{Component, Editor, KeybindingsManager, StdinBuffer, TUI_KEYBINDINGS};
use std::sync::{Arc, Mutex};

fn feed(editor: &mut Editor, data: &str) {
    let mut buffer = StdinBuffer::new();
    for event in buffer.process(data) {
        editor.handle_input(&event);
    }
}

#[test]
fn editor_shift_enter_inserts_newline_and_enter_submits() {
    let mut editor = Editor::new(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()));
    feed(&mut editor, "hello");
    feed(&mut editor, "\x1b[13;2u");
    feed(&mut editor, "world");
    assert_eq!(editor.text(), "hello\nworld");

    let submitted = Arc::new(Mutex::new(None));
    let submitted_for_callback = Arc::clone(&submitted);
    editor.set_on_submit(Box::new(move |text| {
        *submitted_for_callback.lock().unwrap() = Some(text.to_string());
    }));
    feed(&mut editor, "\r");
    assert_eq!(submitted.lock().unwrap().as_deref(), Some("hello\nworld"));
    assert_eq!(editor.text(), "");
}

#[test]
fn editor_wraps_to_width_and_keeps_lines_bounded() {
    let mut editor = Editor::new(KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()));
    feed(&mut editor, "abcdef");
    for line in editor.render(4) {
        assert!(pi_tui::visible_width(&line) <= 4);
    }
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-tui --test input_component
cargo test -p pi-tui --test editor_component
```

Expected: FAIL because `Input` and `Editor` do not exist.

- [ ] **Step 4: Implement reusable editing helpers**

Create:

- `KillRing` with `push(text, prepend, accumulate)`, `yank()`, and `yank_pop()`.
- `UndoStack<T>` with `push(state)`, `undo(current)`, and a fixed maximum of 100 entries.
- `find_word_backward(text, cursor)` and `find_word_forward(text, cursor)` using Unicode whitespace and punctuation boundaries.

Add unit tests inside each helper module for the exact behavior used by `Input` and `Editor`.

- [ ] **Step 5: Implement `Input`**

Required public API:

```rust
pub struct Input {
    // private fields
}

impl Input {
    pub fn new(keybindings: KeybindingsManager) -> Self;
    pub fn value(&self) -> &str;
    pub fn set_value(&mut self, value: impl Into<String>);
    pub fn set_on_submit(&mut self, callback: Box<dyn FnMut(&str)>);
    pub fn set_on_escape(&mut self, callback: Box<dyn FnMut()>);
}
```

`handle_input()` must process:

- paste insertion;
- submit;
- escape/cancel;
- left/right/home/end;
- word left/right;
- backspace/delete;
- delete word backward/forward;
- delete to line start/end;
- yank/yank-pop;
- undo;
- Kitty printable input;
- regular printable Unicode input.

- [ ] **Step 6: Implement `Editor`**

Required public API:

```rust
pub struct Editor {
    // private fields
}

impl Editor {
    pub fn new(keybindings: KeybindingsManager) -> Self;
    pub fn text(&self) -> &str;
    pub fn set_text(&mut self, text: impl Into<String>);
    pub fn set_on_submit(&mut self, callback: Box<dyn FnMut(&str)>);
    pub fn set_on_scroll_page_up(&mut self, callback: Box<dyn FnMut()>);
    pub fn set_on_scroll_page_down(&mut self, callback: Box<dyn FnMut()>);
}
```

For M6, implement:

- multi-line buffer;
- shift-enter newline;
- enter submit and clear text;
- paste insertion;
- wrapping through existing width utilities;
- cursor marker when focused;
- up/down line navigation based on visible columns;
- page up/down callbacks.

- [ ] **Step 7: Export components**

Update `components/mod.rs` and `lib.rs` to export `Input` and `Editor`.

- [ ] **Step 8: Run component tests**

Run:

```bash
cargo test -p pi-tui --test input_component
cargo test -p pi-tui --test editor_component
cargo test -p pi-tui
```

Expected: all pass.

## Task 6: SelectList and Markdown components

**Files:**
- Modify: `crates/pi-tui/Cargo.toml`
- Create: `crates/pi-tui/src/components/select_list.rs`
- Create: `crates/pi-tui/src/components/markdown.rs`
- Modify: `crates/pi-tui/src/components/mod.rs`
- Modify: `crates/pi-tui/src/lib.rs`
- Test: `crates/pi-tui/tests/select_list.rs`
- Test: `crates/pi-tui/tests/markdown.rs`

- [ ] **Step 1: Write failing SelectList tests**

Create `crates/pi-tui/tests/select_list.rs`:

```rust
use pi_tui::{Component, KeybindingsManager, SelectItem, SelectList, StdinBuffer, TUI_KEYBINDINGS};

fn feed(list: &mut SelectList, data: &str) {
    let mut buffer = StdinBuffer::new();
    for event in buffer.process(data) {
        list.handle_input(&event);
    }
}

#[test]
fn select_list_wraps_selection_and_filters_items() {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let mut list = SelectList::new(
        vec![
            SelectItem::new("read", "read").description("Read a file"),
            SelectItem::new("write", "write").description("Write a file"),
        ],
        5,
        keybindings,
    );

    feed(&mut list, "\x1b[A");
    assert_eq!(list.selected_item().unwrap().value, "write");
    list.set_filter("r");
    assert_eq!(list.selected_item().unwrap().value, "read");
}

#[test]
fn select_list_renders_bounded_lines() {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let mut list = SelectList::new(
        vec![SelectItem::new("very-long-command-name", "very-long-command-name").description("long description")],
        5,
        keybindings,
    );

    for line in list.render(12) {
        assert!(pi_tui::visible_width(&line) <= 12);
    }
}
```

- [ ] **Step 2: Write failing Markdown tests**

Create `crates/pi-tui/tests/markdown.rs`:

```rust
use pi_tui::{Component, Markdown};

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
    let mut markdown = Markdown::new("A long paragraph with **bold** text and `inline code` that must wrap.");
    for line in markdown.render(18) {
        assert!(
            pi_tui::visible_width(&line) <= 18,
            "line exceeded width: {:?}",
            line
        );
    }
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-tui --test select_list
cargo test -p pi-tui --test markdown
```

Expected: FAIL because components do not exist.

- [ ] **Step 4: Add Markdown dependency**

Update `crates/pi-tui/Cargo.toml`:

```toml
pulldown-cmark = "0.12"
```

- [ ] **Step 5: Implement `SelectList`**

Required API:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

impl SelectItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self;
    pub fn description(self, description: impl Into<String>) -> Self;
}

pub struct SelectList {
    // private fields
}
```

Implement filtering, selected index movement, page movement, confirm/cancel callbacks, and
width-safe rendering from the spec.

- [ ] **Step 6: Implement `Markdown`**

Required API:

```rust
pub struct Markdown {
    text: String,
    padding_x: usize,
    padding_y: usize,
}

impl Markdown {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn with_padding(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self;
    pub fn set_text(&mut self, text: impl Into<String>);
}
```

Use `pulldown-cmark` events to render headings, paragraphs, lists, quotes, code blocks, inline code,
links, emphasis, strong text, and horizontal rules. Use existing ANSI-aware wrapping utilities for
every non-empty line.

- [ ] **Step 7: Export components and run tests**

Run:

```bash
cargo test -p pi-tui --test select_list
cargo test -p pi-tui --test markdown
cargo test -p pi-tui
```

Expected: all pass.

## Task 7: Interactive CLI route and app skeleton

**Files:**
- Modify: `crates/pi-coding-agent/Cargo.toml`
- Modify: `crates/pi-coding-agent/src/lib.rs`
- Create: `crates/pi-coding-agent/src/interactive/mod.rs`
- Create: `crates/pi-coding-agent/src/interactive/app.rs`
- Create: `crates/pi-coding-agent/src/interactive/transcript.rs`
- Create: `crates/pi-coding-agent/src/interactive/components/mod.rs`
- Create: `crates/pi-coding-agent/src/interactive/components/user_message.rs`
- Create: `crates/pi-coding-agent/src/interactive/components/assistant_message.rs`
- Create: `crates/pi-coding-agent/src/interactive/components/tool_execution.rs`
- Create: `crates/pi-coding-agent/src/interactive/components/footer.rs`
- Test: `crates/pi-coding-agent/tests/interactive_args.rs`
- Test: `crates/pi-coding-agent/tests/interactive_transcript.rs`

- [ ] **Step 1: Write failing CLI route tests**

Create `crates/pi-coding-agent/tests/interactive_args.rs`:

```rust
use pi_coding_agent::{CliRunOptions, run_cli_with_options};

#[tokio::test]
async fn default_invocation_routes_to_interactive_instead_of_unsupported_mode() {
    let output = run_cli_with_options(Vec::<String>::new(), CliRunOptions::default()).await;
    assert_ne!(output.stderr, "unsupported mode: interactive\n");
}

#[tokio::test]
async fn print_mode_still_requires_prompt() {
    let output = run_cli_with_options(vec!["-p".to_string()], CliRunOptions::default()).await;
    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("missing prompt"));
}
```

- [ ] **Step 2: Write failing transcript tests**

Create `crates/pi-coding-agent/tests/interactive_transcript.rs`:

```rust
use pi_coding_agent::interactive::{Transcript, TranscriptItem};

#[test]
fn transcript_scrolls_within_bounds() {
    let mut transcript = Transcript::new();
    for i in 0..20 {
        transcript.push(TranscriptItem::user(format!("message {i}")));
    }
    transcript.scroll_page_up(5);
    assert_eq!(transcript.scroll_offset(), 5);
    transcript.scroll_page_down(2);
    assert_eq!(transcript.scroll_offset(), 3);
    transcript.scroll_to_bottom();
    assert_eq!(transcript.scroll_offset(), 0);
}
```

- [ ] **Step 3: Run failing tests**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_args
cargo test -p pi-coding-agent --test interactive_transcript
```

Expected: FAIL because interactive module and route do not exist.

- [ ] **Step 4: Add dependency and module**

Update `crates/pi-coding-agent/Cargo.toml`:

```toml
pi-tui = { path = "../pi-tui" }
```

Update `crates/pi-coding-agent/src/lib.rs`:

```rust
pub mod interactive;
```

- [ ] **Step 5: Route default invocation**

Replace the current unsupported interactive branch:

```rust
if !parsed.print && !parsed.mode_explicit {
    return CliOutput::failure(CliError::UnsupportedMode("interactive".into()));
}
```

with:

```rust
if !parsed.print && !parsed.mode_explicit {
    return interactive::run_interactive_mode(parsed, options).await;
}
```

For non-TTY test contexts, `run_interactive_mode` should return:

```text
interactive mode requires a TTY
```

with exit code 1, not `unsupported mode: interactive`.

- [ ] **Step 6: Implement transcript model**

In `interactive/transcript.rs`, define:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptItem {
    User { text: String },
    Assistant { id: String, markdown: String, done: bool },
    Tool { call_id: String, name: String, args: serde_json::Value, result: Option<String>, is_error: bool },
    Error { text: String },
}

pub struct Transcript {
    items: Vec<TranscriptItem>,
    scroll_offset: usize,
}
```

Add `push`, `items`, `scroll_page_up`, `scroll_page_down`, `scroll_to_bottom`, and
`scroll_offset`.

- [ ] **Step 7: Implement minimal app shell**

`interactive/app.rs` should expose:

```rust
pub struct InteractiveModeOptions {
    pub terminal_required: bool,
}

pub async fn run_interactive_mode(parsed: CliArgs, options: CliRunOptions) -> CliOutput;
```

For this task, it may only validate TTY presence, construct empty app state, and return a clear
error in test mode. The full event loop lands in Task 9.

- [ ] **Step 8: Run tests**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_args
cargo test -p pi-coding-agent --test interactive_transcript
```

Expected: PASS.

## Task 8: Agent event bridge for interactive transcript

**Files:**
- Create: `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- Modify: `crates/pi-coding-agent/src/interactive/mod.rs`
- Modify: `crates/pi-coding-agent/src/interactive/transcript.rs`
- Test: `crates/pi-coding-agent/tests/interactive_event_bridge.rs`

- [ ] **Step 1: Write failing event bridge tests**

Create `crates/pi-coding-agent/tests/interactive_event_bridge.rs`:

```rust
use pi_agent_core::{AgentEvent, AgentToolResult};
use pi_ai::types::{AssistantMessage, ContentBlock, StopReason, Usage};
use pi_coding_agent::interactive::{InteractiveEventBridge, UiEvent};

#[test]
fn text_delta_updates_assistant_markdown() {
    let mut bridge = InteractiveEventBridge::new();
    let events = bridge.handle(&AgentEvent::LlmEvent {
        event: pi_ai::types::AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hello".to_string(),
            partial: serde_json::json!({}),
        },
    });
    assert_eq!(events, vec![UiEvent::AssistantDelta { text: "hello".to_string() }]);
}

#[test]
fn tool_events_map_to_start_and_end_rows() {
    let mut bridge = InteractiveEventBridge::new();
    let start = bridge.handle(&AgentEvent::ToolCallStart {
        id: "tool_1".to_string(),
        name: "read".to_string(),
        args: serde_json::json!({"path":"Cargo.toml"}),
    });
    assert_eq!(start.len(), 1);

    let end = bridge.handle(&AgentEvent::ToolCallEnd {
        id: "tool_1".to_string(),
        result: AgentToolResult {
            content: vec![ContentBlock::Text { text: "ok".to_string(), cache_control: None }],
            is_error: false,
            terminate: false,
        },
    });
    assert_eq!(end.len(), 1);
}

#[test]
fn agent_done_marks_assistant_complete() {
    let mut bridge = InteractiveEventBridge::new();
    let message = AssistantMessage {
        content: vec![ContentBlock::Text { text: "done".to_string(), cache_control: None }],
        model: "faux".to_string(),
        provider: "faux".to_string(),
        api: "faux".to_string(),
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        id: None,
    };
    let events = bridge.handle(&AgentEvent::AgentDone { message });
    assert!(events.contains(&UiEvent::AssistantDone));
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_event_bridge
```

Expected: FAIL because the bridge does not exist.

- [ ] **Step 3: Implement UI event types**

In `interactive/event_bridge.rs`, define:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    AgentStarted,
    TurnStarted,
    AssistantDelta { text: String },
    AssistantDone,
    ToolStarted { call_id: String, name: String, args: serde_json::Value },
    ToolFinished { call_id: String, result: String, is_error: bool },
    AgentError { error: String },
    CompactionNotice { summary: String },
}

pub struct InteractiveEventBridge {
    // private state for current assistant/tool rows
}
```

Map existing `AgentEvent` variants to `UiEvent` without changing `pi-agent-core`.

- [ ] **Step 4: Apply UI events to transcript**

Add `Transcript::apply_event(event: UiEvent)`:

- `AssistantDelta` appends to the current assistant row or creates one.
- `AssistantDone` marks current assistant row done.
- `ToolStarted` creates a tool row.
- `ToolFinished` updates the matching tool row.
- `AgentError` creates an error row.
- `CompactionNotice` creates a compact assistant/system row.

- [ ] **Step 5: Run bridge tests**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_event_bridge
cargo test -p pi-coding-agent --test interactive_transcript
```

Expected: both pass.

## Task 9: Interactive app event loop

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`
- Modify: `crates/pi-coding-agent/src/interactive/components/*.rs`
- Test: `crates/pi-coding-agent/tests/interactive_mode.rs`
- Test: `crates/pi-coding-agent/tests/interactive_abort.rs`
- Test: `crates/pi-coding-agent/tests/interactive_sessions.rs`

- [ ] **Step 1: Write failing scripted interactive test**

Create `crates/pi-coding-agent/tests/interactive_mode.rs`:

```rust
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_coding_agent::interactive::test_harness::run_scripted_interactive;

#[tokio::test]
async fn scripted_interactive_prompt_renders_assistant_text() {
    let provider = FauxProvider::new(vec![FauxResponse::text("hello from tui")]);
    let output = run_scripted_interactive(provider, "say hi\r").await.unwrap();
    assert!(output.contains("say hi"));
    assert!(output.contains("hello from tui"));
}
```

- [ ] **Step 2: Write failing abort test**

Create `crates/pi-coding-agent/tests/interactive_abort.rs`:

```rust
use pi_coding_agent::interactive::test_harness::run_scripted_idle_interactive;

#[tokio::test]
async fn ctrl_c_exits_when_idle_with_empty_editor() {
    let output = run_scripted_idle_interactive("\x03").await.unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.terminal_restored);
}
```

- [ ] **Step 3: Write failing session test**

Create `crates/pi-coding-agent/tests/interactive_sessions.rs`:

```rust
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_coding_agent::interactive::test_harness::run_scripted_interactive_with_session_dir;

#[tokio::test]
async fn interactive_mode_appends_to_session() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![FauxResponse::text("saved")]);
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "persist me\r")
        .await
        .unwrap();
    assert!(result.session_file.exists());
    let contents = std::fs::read_to_string(result.session_file).unwrap();
    assert!(contents.contains("persist me"));
    assert!(contents.contains("saved"));
}
```

- [ ] **Step 4: Run failing tests**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_abort
cargo test -p pi-coding-agent --test interactive_sessions
```

Expected: FAIL because the app loop and harness do not exist.

- [ ] **Step 5: Implement transcript components**

Each component implements `pi_tui::Component`:

- `UserMessageComponent` renders submitted prompt text.
- `AssistantMessageComponent` owns a `pi_tui::Markdown` and updates it with assistant text.
- `ToolExecutionComponent` renders:
  - running: `tool <name> <call_id> running`
  - success: `tool <name> <call_id> done`
  - error: `tool <name> <call_id> error`
  - one truncated result preview line when result text is available.
- `FooterComponent` renders one line containing cwd, model id, session id/no-session, and status.

Every component test must assert `visible_width(line) <= width`.

- [ ] **Step 6: Implement scripted harness**

Under `interactive/app.rs` or a `#[cfg(test)]` module, expose:

```rust
pub mod test_harness {
    pub async fn run_scripted_interactive(
        provider: pi_ai::providers::faux::FauxProvider,
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, crate::CliError>;

    pub async fn run_scripted_idle_interactive(
        input: &str,
    ) -> Result<ScriptedInteractiveOutput, crate::CliError>;
}
```

The harness should use `VirtualTerminal`, `StdinBuffer`, injected faux provider/model, and an
in-memory render loop. It must not require a real TTY.

- [ ] **Step 7: Implement submit flow**

On editor submit:

1. Push `TranscriptItem::User`.
2. Build `SessionPromptOptions` from parsed CLI args, selected model, existing tools, resources,
   session target, and submitted prompt.
3. Spawn `run_session_prompt(options, Some(&mut on_event))`.
4. In `on_event`, bridge `AgentEvent` into `UiEvent` and send over a channel.
5. UI loop receives `UiEvent`, applies it to transcript, and calls `request_render()`.
6. On success, mark status idle. On error, push `TranscriptItem::Error` and mark status idle.

- [ ] **Step 8: Implement Ctrl+C behavior**

When the editor receives cancel:

- if agent task is running, call abort handle and keep TUI open;
- else if editor text is non-empty, clear it;
- else stop the UI loop and restore terminal state.

Expose an abort handle from the spawned prompt task. If the current `run_session_prompt` path does
not expose `Agent::abort`, add a narrow wrapper that owns `Agent` and returns an abort handle without
changing JSON/RPC behavior.

- [ ] **Step 9: Implement scrolling**

`Transcript` owns `scroll_offset`. Editor page-up/page-down callbacks call:

```rust
transcript.scroll_page_up(viewport_rows);
transcript.scroll_page_down(viewport_rows);
```

Rendering takes the bottom `height - editor_height - footer_height` lines minus `scroll_offset`.

- [ ] **Step 10: Run interactive tests**

Run:

```bash
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_abort
cargo test -p pi-coding-agent --test interactive_sessions
cargo test -p pi-coding-agent
```

Expected: all pass.

## Task 10: Final verification and manual smoke

**Files:**
- No required file changes unless verification exposes bugs.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Run focused crate tests**

Run:

```bash
cargo test -p pi-tui
cargo test -p pi-coding-agent
```

Expected: PASS.

- [ ] **Step 3: Run workspace tests and check**

Run:

```bash
cargo test --workspace
cargo check --workspace
```

Expected: PASS.

- [ ] **Step 4: Manual tmux smoke with controlled terminal**

From `pi-rust/`, run:

```bash
tmux new-session -d -s pi-rust-m6 -x 100 -y 30
tmux send-keys -t pi-rust-m6 "cargo run -p pi-coding-agent" Enter
sleep 2
tmux capture-pane -t pi-rust-m6 -p
tmux send-keys -t pi-rust-m6 "Say exactly: ok" Enter
sleep 2
tmux capture-pane -t pi-rust-m6 -p
tmux send-keys -t pi-rust-m6 C-c
tmux kill-session -t pi-rust-m6
```

Expected:

- first capture shows the prompt editor and footer;
- second capture shows the submitted prompt and either a faux/test response or a provider/auth error row without terminal corruption;
- Ctrl+C exits or aborts according to running/idle state;
- shell prompt is not left in raw mode.

- [ ] **Step 5: Inspect git diff**

Run:

```bash
git status --short
git diff --stat
git diff --check
```

Expected:

- only M6 implementation files are modified;
- no whitespace errors;
- no unrelated `pi/` repo changes are included.
