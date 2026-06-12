# pi-tui interaction roadmap

> Status: draft for implementation planning
> Date: 2026-06-13
> Scope: make the Rust interactive TUI feel stable and respectful of the user's terminal scrollback.

This roadmap narrows M6 from "interactive mode exists" to "interactive mode behaves like a real
terminal application". The immediate user-visible problems are:

- after typing a character, the cursor can jump to the left edge;
- starting the TUI clears or homes the terminal, moving the UI to the top of the window;
- rendering still feels like a proof of concept instead of an inline coding-agent TUI like pi,
  Codex, or Claude Code.

The default target is an inline TUI that appends below the shell prompt and owns only the rows it
has drawn. It must not use the alternate screen by default, must not clear prior scrollback, and
must not move to terminal home during startup.

## Design Principles

1. **Never touch history by default.** Startup, resize, and redraw must not clear the full screen,
   clear scrollback, or move to row 0. Any full redraw is scoped to rows owned by the TUI.
2. **Separate model state from terminal cursor state.** Keep logical content rows, visible viewport
   rows, render-origin rows, and hardware cursor row/column as distinct state.
3. **Make cursor placement a post-render contract.** Components emit `CURSOR_MARKER`; the renderer
   strips it, writes visible text, moves the hardware cursor to the marker, and flushes that move.
4. **Prefer deterministic tests before manual tuning.** Every bug report should first become a
   `VirtualTerminal` or scripted interactive test. Manual tmux smoke tests are secondary evidence.
5. **Keep `pi-tui` app-neutral.** Terminal surfaces, render diffing, cursor behavior, key parsing,
   and reusable editor components stay in `pi-tui`; coding-agent transcript and session wiring stay
   in `pi-coding-agent`.

## Approach Decision

Considered approaches:

- **A. Patch individual symptoms.** Fastest for the current cursor and startup bugs, but it keeps
  the renderer's global clear/full-screen assumptions and will regress under resize or transcript
  growth.
- **B. Move to an alternate-screen app.** Easy to reason about because the app owns the whole
  screen, but it violates the desired pi/Codex/Claude-style inline behavior and hides previous
  terminal context.
- **C. Introduce an inline owned-region renderer.** More work, but it directly matches the desired
  behavior: previous scrollback remains intact, redraws are scoped, and cursor state can be tested.

Recommendation: use C as the renderer foundation, with A only for minimal emergency fixes that are
covered by tests and then folded back into the owned-region model.

## Milestones

### TUI-0: Capture Current Bugs as Failing Tests

Goal: make the current rough behavior reproducible before changing architecture.

Work:

- Add `VirtualTerminal` state tracking for row, column, cursor visibility, clear-screen calls, and
  writes, not only an operation log.
- Add a renderer test proving first render does not call `ClearScreen`, terminal home, or any
  scrollback clear in inline mode.
- Add an interactive test that types `a` into an empty prompt and asserts final cursor column is
  after the prompt prefix and typed character, not column 0.
- Add a test that marker-only cursor movement flushes after `MoveToColumn`.
- Add a resize test that asserts width changes trigger a scoped redraw, not a global clear.

Acceptance:

- The tests fail against the current implementation for the startup clear/home behavior.
- The cursor-jump test records the exact terminal operations needed to explain the bug.
- No real terminal or provider key is required.

Suggested commands:

```bash
cargo test -p pi-tui --test tui_render
cargo test -p pi-coding-agent --test interactive_mode
```

### TUI-1: Inline Owned-Region Rendering

Goal: make `Tui` own only the rows it has emitted, preserving everything above it.

Work:

- Add a renderer configuration, for example `RenderSurface::Inline`, and make it the default for
  `ProcessTerminal`.
- Replace global `clear_screen()` on first/full redraw with a scoped redraw:
  - move from current hardware cursor to the first owned row;
  - clear only owned rows with carriage return + line clear;
  - rewrite owned rows with `\r\n`;
  - leave scrollback above the owned region untouched.
- Track `owned_rows`, `hardware_cursor_row`, `hardware_cursor_col`, and whether the TUI has rendered
  at least once.
- On content growth beyond the viewport, let the terminal scroll naturally and update the owned
  region's visible origin.
- On content shrink, clear leftover owned rows only; do not clear unrelated terminal content.
- Keep a test-only `FullScreen` or `Clearing` surface if existing tests need old behavior, but never
  use it for interactive mode by default.

Acceptance:

- Starting interactive mode appends below existing terminal content.
- No default startup path emits `ClearScreen`, `MoveTo(0,0)`, `ESC[2J`, or `ESC[3J`.
- Width changes and shrink redraws are scoped to owned rows.
- Existing `cargo test -p pi-tui` remains green.

### TUI-2: Cursor Stability Contract

Goal: typed input never leaves the visible cursor at the left edge unless the logical cursor is
actually at column 0.

Work:

- Make `position_hardware_cursor` flush after row/column movement and visibility changes.
- Track final cursor column in `VirtualTerminal`; assert the last visible cursor position after
  each render.
- Ensure differential rendering uses the actual hardware cursor row/column as the starting point,
  not the logical end-of-content row.
- Treat `CURSOR_MARKER` as zero-width through wrapping, truncation, and prompt-prefix composition.
- Add prompt-specific tests for:
  - typing first ASCII character;
  - typing CJK and emoji characters;
  - backspace to empty prompt;
  - moving left/right once editor movement exists;
  - marker moving while rendered text is otherwise unchanged.
- Keep hardware cursor hidden when no focused marker exists.

Acceptance:

- After typing `a` at the prompt, final cursor column is prompt prefix width + 1.
- No render ends with cursor column 0 unless the editor logical cursor is at the start.
- Cursor movement tests pass for ANSI-styled and wide Unicode text.

### TUI-3: Render Scheduling and Input Loop Smoothness

Goal: avoid flicker and redundant redraws during fast input or streaming model output.

Work:

- Replace direct `render_tui()` calls in the interactive loop with a real render scheduler that
  coalesces `request_render()` calls and caps normal renders near 60 Hz.
- Keep forced renders for lifecycle boundaries: first frame, terminal resize, stop/cleanup, and
  prompt completion.
- Add a "no-op input" path that does not render when neither component state nor cursor position
  changes.
- Batch adjacent assistant deltas before rendering when events arrive faster than the render
  interval.
- Add tests that many input chunks or assistant deltas produce bounded render counts.

Acceptance:

- Holding a key or receiving many streaming deltas does not cause one full terminal write per byte.
- Cursor still updates promptly for local editing.
- Render scheduler behavior is deterministic under test clocks.

### TUI-4: Prompt Editor Usability

Goal: make the prompt editor comfortable enough for real coding-agent use.

Work:

- Complete editor movement: left/right, up/down across wrapped visual lines, home/end,
  word-left/word-right, page-up/page-down handoff to transcript scrolling.
- Complete editing operations: delete forward/backward, word deletion, kill/yank, undo/redo,
  multiline paste, Shift+Enter newline, Enter submit.
- Add horizontal handling for single-line input and visual-line handling for multiline editor.
- Add an optional fake cursor style for terminals where hardware cursor positioning is unreliable,
  while keeping hardware cursor marker for IME positioning.
- Add tests for grapheme-safe cursor movement and deletion, including CJK, emoji, combining marks,
  and ANSI text around the marker.

Acceptance:

- Editor tests describe every default keybinding action that M6 exposes.
- Cursor visual position and logical cursor byte index remain consistent after all editor actions.
- Pasted multiline text does not corrupt render layout.

### TUI-5: Transcript Layout and Scrolling

Goal: keep the prompt area stable while transcript content grows.

Work:

- Split interactive UI into stable regions:
  - transcript viewport;
  - prompt editor;
  - compact footer/status row.
- Keep the prompt editor anchored at the bottom of the owned region.
- Add transcript scroll state with page up/down, bottom lock, and "new output below" handling.
- Render tool start/end rows compactly and truncate noisy output with explicit continuation markers.
- Render Markdown through `pi-tui::Markdown` instead of ad hoc `line` truncation.
- Add tests for transcript growth, scrollback, and prompt focus restoration after scrolling.

Acceptance:

- Streaming output never pushes the prompt cursor into an unexpected row.
- Page up/down scrolls transcript without corrupting the prompt editor.
- Returning to bottom resumes auto-scroll for new assistant/tool output.

### TUI-6: Terminal Lifecycle and Cleanup

Goal: raw mode, bracketed paste, Kitty keyboard negotiation, and exit paths are reliable.

Work:

- Introduce an RAII terminal session guard that starts raw mode and always restores terminal state.
- Drain pending input on exit to avoid delayed key-release events leaking to the shell.
- Make Ctrl+C behavior explicit and tested:
  - running prompt: abort and keep TUI open;
  - idle with text: clear editor;
  - idle empty: restore terminal and exit.
- On normal exit, leave the final inline TUI content in scrollback and put the shell cursor on a new
  clean line.
- On panic/error, restore raw mode and cursor visibility, then print a concise error after the owned
  region.

Acceptance:

- All stop paths record bracketed paste disable, Kitty disable, cursor show, and raw mode restore.
- Exiting does not leave the shell prompt inside the TUI-owned region.
- Scripted Ctrl+C tests pass without hangs.

### TUI-7: Cross-Terminal Manual Smoke Suite

Goal: catch terminal emulator differences that unit tests cannot model.

Work:

- Add a documented smoke script that runs interactive mode in tmux and captures panes before,
  during, and after a prompt.
- Include scenarios for:
  - existing shell output above the TUI;
  - typing first character;
  - typing wide Unicode;
  - resizing narrower and wider;
  - streaming assistant output;
  - Ctrl+C while running and idle.
- Record known behavior for common terminals: wezterm, kitty, iTerm2, Terminal.app, GNOME Terminal,
  tmux, and SSH/tmux if available.

Acceptance:

- Smoke evidence shows previous terminal content remains above the TUI.
- Cursor remains at the prompt after each typed character.
- Resizing does not clear unrelated scrollback.

### TUI-8: Interaction Polish After Stability

Goal: improve feel after the renderer and editor are trustworthy.

Work:

- Add focused visual styling for prompt/status without dominating the terminal.
- Improve Markdown rendering for code fences, lists, block quotes, and links.
- Add compact spinners/progress for running agent and tools.
- Add optional model/session/status menus using `SelectList`.
- Add theme hooks only after the component API is stable.

Acceptance:

- Polish changes do not alter terminal ownership or cursor invariants.
- Each visual change has a width-bounded render test.

## Suggested Execution Order

1. Implement TUI-0 and TUI-1 together, because startup behavior depends on renderer ownership.
2. Implement TUI-2 immediately after, before adding editor features.
3. Implement TUI-3 once cursor and owned-region tests are green.
4. Implement TUI-4 and TUI-5 in small editor/transcript slices.
5. Implement TUI-6 before any broader manual dogfooding.
6. Use TUI-7 as a release gate for calling interactive mode usable.
7. Do TUI-8 only after the stability milestones are green.

## Verification Gate for Every Milestone

Each milestone should finish with:

```bash
cargo fmt --check
cargo test -p pi-tui
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

For milestones that touch terminal behavior, also run the tmux smoke suite from TUI-7 once it
exists.

## Initial Decisions

- `pi-tui` should implement `Inline` as the only production surface for this roadmap. Alternate
  screen support stays out of scope until a real user need appears.
- Normal exit should leave the visible inline transcript in scrollback, matching inline coding-agent
  tools. A compact final summary can be added later as an optional mode.
- Hardware cursor positioning should be enabled by default for IME support. A fake cursor fallback
  can be added for terminals where hardware cursor movement is unreliable.
