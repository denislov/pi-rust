# pi-tui interaction roadmap

> Status: updated — TUI-0 through TUI-6 complete, TUI-8 first slice done
> Last updated: 2026-06-19
> Scope: make the Rust interactive TUI feel stable and respectful of the user's terminal scrollback.

## Current state (2026-06-19)

All stability milestones (TUI-0 through TUI-6) are complete with green tests:
- **TUI-0**: `VirtualTerminal` tracks cursor row/col/visibility/clear-screen count; tests assert no global clear on startup.
- **TUI-1**: `RenderSurface::Inline` is the default; `render_full_inline`/`render_differential_inline` do scoped redraws; `owned_rows`/`hardware_cursor_row`/`rendered_once` tracked; `assert_no_global_clear` tests green.
- **TUI-2**: `CURSOR_MARKER` + `position_hardware_cursor` with flush; cursor-jump bug fixed and tested (`scripted_interactive_keeps_cursor_after_first_typed_character` asserts `cursor_col == 3`; CJK/emoji/backspace/left-arrow tests green).
- **TUI-3**: `RenderScheduler` with request/force/coalescing/`should_render_now`/`mark_rendered`, integrated into interactive app.
- **TUI-4**: `Editor` with full movement/editing (left/right/up/down/home/end/word-nav, delete/word-delete/kill-ring/undo-redo, multiline paste, Shift+Enter newline, Enter submit).
- **TUI-5**: Transcript viewport with scroll state, page up/down, bottom lock, "new output below" indicator, prompt anchor; Markdown via `pi-tui::Markdown`.
- **TUI-6**: RAII terminal session guard (`ProcessTerminal::start`/`stop`), raw mode, bracketed paste, Kitty keyboard negotiation; Ctrl+C three-path tested (abort/clear/exit).

**TUI-8 first slice complete** (2026-06-19): 8-color semantic styling + Markdown polish — `Style`/`paint`/`paint_with`/`color_enabled` primitive, Markdown headings (bold)/inline code (reverse)/code blocks (dim fence+content)/blockquotes (dim)/rules (dim), transcript role coloring (user/system/error/tool/footer). See `docs/superpowers/specs/2026-06-19-pi-tui-tui8-color-and-markdown-design.md`.

**Remaining TUI-8 work**: spinner/progress animation, SelectList menus (model/session/status), theme system (256-color/dark/light).

## Design Principles

1. **Never touch history by default.** Startup, resize, and redraw must not clear the full screen, clear scrollback, or move to row 0. Any full redraw is scoped to rows owned by the TUI.
2. **Separate model state from terminal cursor state.** Keep logical content rows, visible viewport rows, render-origin rows, and hardware cursor row/column as distinct state.
3. **Make cursor placement a post-render contract.** Components emit `CURSOR_MARKER`; the renderer strips it, writes visible text, moves the hardware cursor to the marker, and flushes that move.
4. **Prefer deterministic tests before manual tuning.** Every bug report should first become a `VirtualTerminal` or scripted interactive test. Manual tmux smoke tests are secondary evidence.
5. **Keep `pi-tui` app-neutral.** Terminal surfaces, render diffing, cursor behavior, key parsing, and reusable editor components stay in `pi-tui`; coding-agent transcript and session wiring stay in `pi-coding-agent`.

## Approach Decision (settled)

Chose **C: inline owned-region renderer** — previous scrollback remains intact, redraws are scoped, cursor state is testable. `RenderSurface::Inline` is the only production surface; `Clearing` retained for tests only.

## Milestones

### TUI-0: Capture Current Bugs as Failing Tests — ✅ COMPLETE

`VirtualTerminal` tracks row/column/cursor visibility/clear-screen count. Tests in `tui_render.rs` assert no `ClearScreen`/`ClearFromCursorDown`/`ESC[2J`/`ESC[3J`/`ESC[H` on first render, resize, or shrink. Interactive tests assert cursor column after typing.

### TUI-1: Inline Owned-Region Rendering — ✅ COMPLETE

`RenderSurface::Inline` is the default. `render_full_inline` and `render_differential_inline` do scoped redraws using `owned_rows`, `previous_viewport_top`, `hardware_cursor_row/col`. No global clear on startup/resize/shrink. Tests in `tui_render.rs`: `first_render_appends_inline_without_clearing_or_homing`, `width_change_triggers_scoped_redraw_without_global_clear`, `shrink_with_clear_on_shrink_clears_only_owned_rows`.

### TUI-2: Cursor Stability Contract — ✅ COMPLETE

`CURSOR_MARKER` (`\x1b_pi:c\x07`) is zero-width; `extract_cursor_marker` strips it and returns `CursorPosition`. `position_hardware_cursor` flushes after movement. Tests: `scripted_interactive_keeps_cursor_after_first_typed_character` (cursor_col==3), `scripted_interactive_positions_cursor_after_wide_unicode` (CJK/emoji cursor_col==4), `scripted_interactive_backspace_returns_cursor_to_prompt_start`, `scripted_interactive_left_arrow_moves_cursor_within_prompt`.

### TUI-3: Render Scheduling and Input Loop Smoothness — ✅ COMPLETE

`RenderScheduler` (`runtime.rs`) with `request(force)`/`has_pending`/`next_render_at`/`should_render_now`/`mark_rendered`. Min interval ~16ms (60Hz). Force renders for lifecycle boundaries. Integrated into `run_interactive_loop` via `schedule_render`/`pending_render_delay`. Test: `scripted_interactive_coalesces_fast_typed_input_renders` asserts bounded render count.

### TUI-4: Prompt Editor Usability — ✅ COMPLETE

`Editor` component (`components/editor.rs`, 695 lines) with: left/right/up/down across wrapped visual lines, home/end, word-left/word-right, page-up/page-down handoff, delete forward/backward, word deletion, kill-ring (yank), undo/redo, multiline paste, Shift+Enter newline, Enter submit. Grapheme-safe cursor movement. Tests in `editor_component.rs` (285 lines).

### TUI-5: Transcript Layout and Scrolling — ✅ COMPLETE

`InteractiveRoot` splits into transcript viewport + prompt editor + footer. `Transcript` model with scroll offset, page up/down, bottom lock, `has_new_output_below`. `render_transcript_viewport` handles padding/slicing/indicator. Tool rows compact with truncation. Markdown via `pi-tui::Markdown`. Tests: `scripted_interactive_keeps_prompt_anchored_below_transcript_viewport`, `scripted_interactive_new_output_does_not_unlock_scrolled_transcript`, `render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output`.

### TUI-6: Terminal Lifecycle and Cleanup — ✅ COMPLETE (gaps noted)

`ProcessTerminal::start` enables raw mode + bracketed paste (`\x1b[?2004h`) + Kitty keyboard (`\x1b[>7u\x1b[?u\x1b[c`) + hide cursor. `stop` disables all + shows cursor + restores raw mode. Ctrl+C three-path tested: running→abort+keep TUI, idle+text→clear editor, idle+empty→exit. Tests: `ctrl_c_aborts_running_prompt_and_keeps_tui_open`, `ctrl_c_exits_when_idle_with_empty_editor`.

**Known gaps** (not blocking): panic-path raw-mode restoration not explicitly tested; exit-time input drain (`drain_input`) is a no-op stub on `ProcessTerminal`. These are low-risk but worth a follow-up pass.

### TUI-7: Cross-Terminal Manual Smoke Suite — ❌ NOT STARTED

Goal: catch terminal emulator differences that unit tests cannot model.

Work:

- Add a documented smoke script that runs interactive mode in tmux and captures panes before, during, and after a prompt.
- Include scenarios for:
  - existing shell output above the TUI;
  - typing first character;
  - typing wide Unicode;
  - resizing narrower and wider;
  - streaming assistant output;
  - Ctrl+C while running and idle.
- Record known behavior for common terminals: wezterm, kitty, iTerm2, Terminal.app, GNOME Terminal, tmux, and SSH/tmux if available.

Acceptance:

- Smoke evidence shows previous terminal content remains above the TUI.
- Cursor remains at the prompt after each typed character.
- Resizing does not clear unrelated scrollback.

### TUI-8: Interaction Polish After Stability — 🟡 IN PROGRESS

**First slice complete** (2026-06-19): 8-color semantic styling + Markdown rendering polish.

Done:
- `Style`/`Color`/`paint`/`paint_with`/`color_enabled` primitive in `pi-tui/src/style.rs` with NO_COLOR/TERM=dumb support.
- Markdown: headings bold, inline code reverse, code blocks dim fence+content, blockquotes dim, rules dim.
- Transcript: user (cyan), system (dim), error (red bold), tool name (yellow), tool status (yellow/red/dim), footer status/cwd/usage colored.
- Spec: `docs/superpowers/specs/2026-06-19-pi-tui-tui8-color-and-markdown-design.md`.
- Plan: `docs/superpowers/plans/2026-06-19-pi-tui-tui8-color-and-markdown.md`.

Remaining:
- **Spinner/progress** for running agent and tools — compact animation in the footer or tool row.
- **SelectList menus** — optional model/session/status switcher using `pi-tui::SelectList`.
- **Theme system** — 256-color/dark/light/custom palettes with capability detection (deferred until component API stable).

Acceptance (per remaining slice):
- Polish changes do not alter terminal ownership or cursor invariants.
- Each visual change has a width-bounded render test.

## Verification Gate for Every Milestone

```bash
cargo fmt --check
cargo test -p pi-tui
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

For milestones that touch terminal behavior, also run the tmux smoke suite from TUI-7 once it exists.

## Initial Decisions (settled)

- `pi-tui` implements `Inline` as the only production surface. Alternate screen support stays out of scope until a real user need appears.
- Normal exit leaves the visible inline transcript in scrollback, matching inline coding-agent tools.
- Hardware cursor positioning enabled by default for IME support.
- 8-color semantic mapping (not 256/true-color or themes) for the first polish slice — cross-terminal consistency.
- NO_COLOR + TERM=dumb for disabling color — follows industry convention.
