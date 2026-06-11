# Design: Rust M6 interactive TUI

- Date: 2026-06-11
- Status: Draft (pending review)
- Scope: M6 of the Rust port ROADMAP - an interactive terminal UI path for `pi-coding-agent` backed by `pi-tui`.
- Depends on: the existing `pi-tui` renderer PoC, M1 built-in tools, M3 JSONL sessions, M4 harness controls, and M5 headless event adaptation already present in `pi-coding-agent`.

## 1. Context

`ROADMAP.md` defines M6 as the first Rust interactive TUI milestone:

- add the `pi-tui` input stack: raw mode, stdin buffering, Kitty keyboard protocol, key parsing, and keybindings;
- add interactive components: `Input`, `Editor`, `SelectList`, and `Markdown`;
- add focus, overlay, and an asynchronous render loop;
- connect `pi-coding-agent` interactive mode to `pi-tui`;
- accept the phase when a user can start an interactive session, type, scroll, and view Markdown/tool output.

The current Rust `pi-tui` crate has the rendering foundation: `Component`, `Container`,
`Text`, `Spacer`, `Tui<T: Terminal>`, `ProcessTerminal`, `VirtualTerminal`, width utilities,
line-width validation, synchronized output, and differential rendering. It intentionally does not
yet own terminal input or long-running UI state.

The current Rust `pi-coding-agent` already has enough headless behavior to make a useful
interactive vertical slice: built-in tools, session persistence, prompt resources, core harness
controls, JSON/RPC protocol types, and a session-backed prompt runner. M6 should reuse those paths
instead of building a separate agent runtime for the TUI.

TypeScript reference files for this phase:

- `pi/packages/tui/src/terminal.ts` - raw terminal lifecycle, bracketed paste, Kitty negotiation, progress/title controls.
- `pi/packages/tui/src/stdin-buffer.ts` - escape sequence framing and paste detection.
- `pi/packages/tui/src/keys.ts` - key identifiers, Kitty CSI-u parsing, key release detection.
- `pi/packages/tui/src/keybindings.ts` - configurable keybinding definitions and conflict detection.
- `pi/packages/tui/src/tui.ts` - focus, overlays, input dispatch, render scheduling, cursor marker extraction.
- `pi/packages/tui/src/components/{input,editor,select-list,markdown}.ts` - component behavior.
- `pi/packages/coding-agent/src/modes/interactive/interactive-mode.ts` - app-level composition and agent event handling.

## 2. Goals and success criteria

Build a usable Rust interactive coding-agent mode, not full TypeScript parity.

M6 is done when:

1. Running `pi-coding-agent` without `-p` and without `--mode` starts an interactive TUI instead of returning `UnsupportedMode("interactive")`.
2. `pi-tui` can start and stop a real terminal session, enter raw mode, enable bracketed paste, install a resize handler, and restore terminal state on normal exit, abort, and panic-safe cleanup paths.
3. `pi-tui` has deterministic input tests covering:
   - legacy arrow, enter, tab, escape, backspace, delete, home/end, page up/down;
   - printable Unicode input;
   - Kitty CSI-u printable keys;
   - Kitty press/repeat/release event parsing and release filtering;
   - batched stdin data split into complete escape sequences;
   - bracketed paste start/end re-emission as one paste event.
4. `pi-tui` exposes configurable keybindings with conflict detection. M6 must include the TUI defaults from the TypeScript package for editor, input, and select-list actions.
5. Focused components receive input through `handle_input`; key release events are ignored unless the component opts in; global input listeners can consume or rewrite input before focus dispatch.
6. The async render loop coalesces `request_render()` calls, avoids rendering faster than about 60 Hz, supports forced full redraw, and remains testable with `VirtualTerminal`.
7. Focus and overlay support cover the M6 app needs:
   - one focused component at a time;
   - overlay handles for hide, set hidden, focus, unfocus, and focus restore;
   - centered and edge-anchored overlay placement;
   - non-capturing overlays for passive UI.
8. Hardware cursor support works for prompt editing: focused editor components emit a zero-width cursor marker, `Tui` strips it before writing, computes the display column with ANSI-aware width utilities, and positions or hides the real cursor according to configuration.
9. `pi-tui` provides M6 component implementations:
   - `Input`: single-line editing with grapheme-safe cursor movement, paste, submit, escape, deletion, word deletion, kill/yank, and undo.
   - `Editor`: multi-line prompt editor with wrapping, vertical cursor movement, shift-enter newline, enter submit, page up/down scroll requests, paste, and cursor marker output.
   - `SelectList`: scrollable selection list with filtering, wrapping selection movement, confirm, cancel, descriptions, and width-safe truncation.
   - `Markdown`: width-safe Markdown rendering for headings, paragraphs, emphasis, inline code, fenced code blocks, block quotes, ordered/unordered lists, links, horizontal rules, and ANSI-styled text. It may use `pulldown-cmark`; syntax highlighting and terminal images are deferred.
10. `pi-coding-agent` interactive mode shows:
    - prior session messages loaded from JSONL when a session target is selected;
    - user prompts as transcript entries;
    - assistant streaming text as Markdown;
    - tool execution start/end rows with tool name, call id, error status, and truncated text content;
    - a prompt editor and compact footer with cwd, model id, session id, and running/idle status.
11. Submitting a prompt in the TUI runs the existing session-backed prompt path, appends resulting messages to the same JSONL v3 session format as print/json/rpc modes, and renders events as they arrive.
12. The user can scroll transcript content with page up/down and return to the prompt editor without corrupting terminal output.
13. Ctrl+C behavior is deterministic:
    - while the agent is running, it aborts the current prompt and leaves the TUI open;
    - while idle with an empty editor, it exits cleanly;
    - while idle with text in the editor, it clears the editor.
14. All tests are deterministic and offline. No test requires a real provider key, network, or a real terminal.

Required verification:

- `cargo fmt --check`
- `cargo test -p pi-tui`
- `cargo test -p pi-coding-agent`
- `cargo test --workspace`
- `cargo check --workspace`

Manual smoke verification:

- Start `pi-coding-agent` in a tmux session with a faux provider or injected test runtime.
- Type a prompt, submit it, observe streamed assistant Markdown.
- Trigger a tool call in the faux script and observe tool start/end rows.
- Page up/down through transcript history.
- Press Ctrl+C while running and while idle, verifying abort and clean terminal restoration.

## 3. Non-goals

M6 does not implement the full TypeScript interactive application.

Deferred to M7 or later:

- extension UI system, extension widgets, extension editors, custom extension shortcuts;
- full slash-command catalog;
- auth and OAuth dialogs beyond simple error text surfaced by the runtime;
- model/provider selectors beyond the generic `SelectList` foundation;
- themes loaded from user config;
- terminal image rendering, image capability detection, clipboard image paste, Kitty image cleanup;
- syntax highlighting in Markdown code blocks;
- full autocomplete and fuzzy provider stack;
- external editor integration;
- HTML export/share UI;
- session tree/fork/clone browsers beyond the session flags already supported by headless mode;
- Windows console parity beyond best-effort VT input handling through `crossterm`.

M6 also does not replace `pi-coding-agent` JSON/RPC modes. Interactive mode may reuse the same
protocol event adapter internally, but the wire protocol remains owned by M5.

## 4. Key decisions

### 4.1 Keep `pi-tui` reusable and app-neutral

Generic terminal primitives, key parsing, keybindings, focus, overlays, and reusable components
belong in `pi-tui`. Coding-agent-specific transcript rows, footer text, tool rendering, session
status, and prompt orchestration belong in `pi-coding-agent`.

This keeps `pi-tui` useful for future crates without coupling it to agent sessions.

### 4.2 Keep the renderer model from the PoC

Do not introduce `ratatui`. M6 continues the PoC decision: components render strings, the framework
owns width validation and terminal writes, and `VirtualTerminal` remains the primary test backend.

M6 can extend `Component` with input/focus hooks, but it should preserve the current render contract:

```rust
fn render(&mut self, width: usize) -> Vec<String>;
```

### 4.3 Parse raw input before mapping to commands

Use a Rust `StdinBuffer` equivalent to the TypeScript implementation. It should split raw stdin
chunks into complete sequences before key parsing. Do not rely only on `crossterm::event` for M6,
because the TypeScript behavior depends on raw sequence fidelity for Kitty CSI-u, bracketed paste,
and partial escape sequences.

### 4.4 Make keybindings configurable through action ids

Components must not hardcode checks such as "ctrl+c means cancel". They should ask a
`KeybindingsManager` whether an input event matches an action id:

```rust
keybindings.matches(&event, "tui.select.cancel")
```

This mirrors the TypeScript rule and keeps downstream application keybindings configurable.

### 4.5 Use a single UI mutation lane

The interactive app should run the model/tool loop in a background task and send UI events over a
channel. Transcript state must be mutated by the UI loop, not from arbitrary tool/model tasks. This
avoids shared mutable UI state and makes rendering deterministic.

### 4.6 Markdown uses a Rust parser, with styling kept narrow

Use `pulldown-cmark` for Markdown parsing. M6 should render width-safe terminal text for common
Markdown constructs. Syntax highlighting, image lines, and full theme parity are later work.

### 4.7 Terminal cleanup is part of the contract

Raw mode and bracketed paste must be restored even when the app returns an error. M6 should add an
RAII-style terminal session guard around `ProcessTerminal::start()` and make tests assert the stop
operations are emitted.

## 5. Architecture

### 5.1 `pi-tui` module layout

Add or extend modules under `crates/pi-tui/src`:

```text
input/
  mod.rs             # InputEvent, InputListener, public exports
  key.rs             # Key, KeyEvent, parse_key, matches_key, Kitty helpers
  keybindings.rs     # KeybindingDefinition, KeybindingsManager, TUI_KEYBINDINGS
  stdin_buffer.rs    # StdinBuffer and bracketed paste framing
runtime.rs           # TuiRuntime, render scheduling, input dispatch
overlay.rs           # OverlayOptions, OverlayHandle, overlay layout
cursor.rs            # CURSOR_MARKER and cursor extraction helper
components/
  input.rs           # single-line Input
  editor.rs          # multi-line Editor
  select_list.rs     # SelectList
  markdown.rs        # Markdown
  truncated_text.rs  # optional helper for transcript rows
  box.rs             # optional bordered panel helper for overlays
kill_ring.rs         # reusable kill/yank ring
undo_stack.rs        # reusable undo stack
word_navigation.rs   # word boundary helpers
```

Existing modules remain:

```text
component.rs
terminal.rs
tui.rs
virtual_terminal.rs
utils/
```

### 5.2 Public API shape

The exact Rust names can be adjusted during implementation, but M6 should converge on this shape:

```rust
pub trait Component {
    fn render(&mut self, width: usize) -> Vec<String>;
    fn handle_input(&mut self, _event: &InputEvent) {}
    fn wants_key_release(&self) -> bool { false }
    fn invalidate(&mut self) {}
}

pub trait Focusable {
    fn set_focused(&mut self, focused: bool);
    fn focused(&self) -> bool;
}

pub const CURSOR_MARKER: &str = "\x1b_pi:c\x07";

pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    Raw(String),
    Resize(TerminalSize),
}

pub struct KeyEvent {
    pub key: Key,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
}

pub enum KeyEventKind {
    Press,
    Repeat,
    Release,
}
```

`Tui<T: Terminal>` should keep `render_once()` for deterministic tests. A new `TuiRuntime<T>` can
own coalescing, input dispatch, and lifecycle:

```rust
pub struct TuiRuntime<T: Terminal> {
    tui: Tui<T>,
    keybindings: KeybindingsManager,
    min_render_interval: Duration,
}
```

`ProcessTerminal` should expose raw lifecycle and terminal controls:

```rust
impl ProcessTerminal {
    pub fn start(&mut self, input_tx: InputSender) -> Result<TerminalSessionGuard, TerminalError>;
    pub fn stop(&mut self) -> Result<(), TerminalError>;
    pub fn drain_input(&mut self, max: Duration, idle: Duration) -> Result<(), TerminalError>;
    pub fn set_title(&mut self, title: &str) -> Result<(), TerminalError>;
    pub fn set_progress(&mut self, active: bool) -> Result<(), TerminalError>;
}
```

### 5.3 Interactive coding-agent layout

Add a focused interactive subtree:

```text
crates/pi-coding-agent/src/interactive/
  mod.rs
  app.rs              # InteractiveApp state and event loop
  event_bridge.rs     # AgentEvent -> UiEvent mapping
  transcript.rs       # transcript model and scroll state
  components/
    mod.rs
    editor.rs         # app-specific prompt editor wrapper if needed
    footer.rs
    assistant_message.rs
    user_message.rs
    tool_execution.rs
```

`run_cli_with_options()` should route default non-print invocations to interactive mode:

```rust
if !parsed.print && !parsed.mode_explicit {
    return interactive::run_interactive_mode(parsed, options).await;
}
```

For tests, expose a non-TTY harness that accepts scripted input and a `VirtualTerminal`, while the
binary uses `ProcessTerminal`.

### 5.4 Event flow

Input flow:

```text
raw stdin bytes
  -> StdinBuffer
  -> InputEvent
  -> global input listeners
  -> focused Component::handle_input
  -> request_render()
  -> Tui::render_once()
```

Prompt flow:

```text
Editor submit
  -> append user transcript row
  -> spawn run_session_prompt(options, on_event)
  -> AgentEvent stream
  -> UiEvent channel
  -> transcript/tool rows update on UI task
  -> request_render()
```

The final session write remains inside `run_session_prompt`, so interactive mode writes the same
JSONL v3 session shape as print/json/rpc modes.

## 6. Component scope

### 6.1 Input

`Input` is a single-line editor. It should support:

- printable Unicode and Kitty printable input;
- bracketed paste insertion;
- grapheme-safe left/right/backspace/delete;
- word left/right and word deletion;
- line start/end;
- kill/yank and yank-pop;
- undo;
- submit and escape callbacks;
- horizontal scrolling and cursor marker emission when focused.

### 6.2 Editor

`Editor` is the prompt editor used by interactive mode. M6 should implement a practical subset:

- multi-line text;
- wrapping to terminal width;
- cursor up/down/left/right and line start/end;
- shift-enter newline;
- enter submit;
- paste;
- page up/down callbacks for transcript scroll when editor is at the top or bottom;
- width-safe rendering with a minimal border or prompt prefix;
- cursor marker output.

Autocomplete, extension shortcuts, external editor, image paste, and all custom TS editor actions
are non-goals for M6.

### 6.3 SelectList

`SelectList` should match the generic TS component behavior:

- item value, label, and optional description;
- filtering by value/label prefix;
- selected index clamped to filtered items;
- up/down wrapping;
- page up/down movement;
- confirm and cancel callbacks;
- scroll indicator when not all items are visible;
- width-safe truncation of primary and description columns.

### 6.4 Markdown

`Markdown` should render:

- paragraphs with ANSI-aware wrapping;
- headings;
- inline code and fenced code blocks;
- block quotes;
- unordered and ordered lists;
- emphasis, strong, strikethrough where supported by the parser;
- links using OSC 8 when enabled and plain fallback text otherwise;
- horizontal rules.

Every rendered non-image line must satisfy `visible_width(line) <= width`. M6 does not render
terminal images.

### 6.5 Coding-agent transcript components

Application components in `pi-coding-agent` should be intentionally simple:

- `UserMessageComponent`: shows submitted prompt text.
- `AssistantMessageComponent`: renders accumulated assistant text through `Markdown`.
- `ToolExecutionComponent`: shows tool name, call id, running/done/error state, and truncated text result.
- `FooterComponent`: shows cwd, model id, session id or "no session", and running/idle/aborted status.
- `TranscriptComponent`: owns scroll offset and composes message components.

## 7. Error handling

- Terminal setup failures return `CliOutput` with non-zero exit and stderr text; they must not panic.
- If terminal output returns `EIO`, `EPIPE`, or `ENOTCONN`, interactive mode exits cleanly after best-effort restore.
- If the agent stream returns an error, the transcript shows an assistant error row and the app returns to idle state.
- If a component renders a line wider than the terminal width, `TuiError::LineTooWide` remains the failure mode. Interactive mode should stop the terminal before surfacing the error.
- Ctrl+C while running calls the existing agent abort path. Ctrl+C while idle follows the behavior in the success criteria.

## 8. Testing strategy

`pi-tui` tests:

- `input_stack.rs`: stdin buffering, paste, key parsing, release filtering.
- `keybindings.rs`: defaults, user overrides, conflict detection.
- `tui_runtime.rs`: render coalescing, forced redraw, focus dispatch, input listeners.
- `overlay.rs`: placement, hide/show, focus restore, non-capturing overlays.
- `cursor.rs`: cursor marker stripping and ANSI-aware column calculation.
- `input_component.rs`: single-line editing, graphemes, paste, kill/yank, undo.
- `editor_component.rs`: multi-line editing, wrapping, submit/newline, cursor marker.
- `select_list.rs`: filtering, selection movement, scroll indicator, truncation.
- `markdown.rs`: Markdown constructs and width guarantees.

`pi-coding-agent` tests:

- `interactive_args.rs`: default CLI route selects interactive, `-p` and `--mode json/rpc` remain unchanged.
- `interactive_event_bridge.rs`: core `AgentEvent` values map to transcript UI events.
- `interactive_transcript.rs`: scroll math and transcript rendering.
- `interactive_mode.rs`: scripted input submits a prompt through the faux provider and renders user/assistant/tool rows.
- `interactive_abort.rs`: Ctrl+C aborts an in-flight prompt and exits when idle with an empty editor.
- `interactive_sessions.rs`: session flags load existing JSONL and new messages append through `run_session_prompt`.

Manual tests use tmux, not CI-only assertions, because raw terminal behavior is host-dependent.

## 9. Rollout plan

Implement M6 in layers:

1. Input parsing and keybindings in `pi-tui`.
2. Terminal lifecycle and render runtime.
3. Focus, cursor marker, and overlays.
4. Components needed by the M6 app.
5. Coding-agent transcript and interactive app shell.
6. Agent event bridge and prompt submission.
7. Manual tmux smoke pass and cleanup.

Each layer should leave `cargo test -p pi-tui` or `cargo test -p pi-coding-agent` green before the
next layer starts.

## 10. Risks

- Raw terminal input is host-sensitive. Keep the parser deterministic and cover partial sequences,
  Kitty CSI-u, and bracketed paste with unit tests.
- The TypeScript `Editor` is large. M6 should implement the prompt-editing subset listed here and
  avoid extension/autocomplete/image behavior until later.
- Render scheduling can become flaky if tied to wall-clock time. Tests should use immediate flush
  hooks or a configurable minimum interval.
- Agent events arrive asynchronously while the user continues typing. Restrict transcript mutation
  to the UI event loop.
- Markdown width bugs can corrupt terminal output. Every Markdown test should assert visible width
  for every rendered line.

