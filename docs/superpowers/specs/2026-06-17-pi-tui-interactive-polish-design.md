# Design: Rust interactive TUI M6 polish (footer, help, key hints, tool readability)

- Date: 2026-06-17
- Status: Draft (pending review)
- Scope: Low-risk polish on top of the existing Rust M6 interactive TUI. No new
  infrastructure, no cross-crate event changes, no theme/ANSI system.
- Depends on: `pi-tui` input/keybindings/runtime/components and the
  `pi-coding-agent` interactive app from M6 (both green as of this date).

## 1. Context

The Rust M6 interactive vertical slice is complete and green:

- `cargo test -p pi-tui` passes (input stack, keybindings, runtime, overlay,
  cursor, components).
- `cargo test -p pi-coding-agent --test interactive_*` passes (args, event
  bridge, transcript, mode, abort, sessions).

What is missing, relative to the TypeScript reference, is the everyday
readability layer: the footer is a bare `status | cwd | model | session` string,
there is no startup help, key hints are not derived from the keybinding manager,
and tool result rows always show a hard 3-line preview with no way to expand.

This milestone closes those four gaps with the smallest possible changes that do
not break the existing layout-sensitive scripted tests.

## 2. Goals and success criteria

1. The footer remains a **single line** and keeps the literal substring
   `status: idle` (and `status: running`) so existing assertions hold. It gains:
   - cwd abbreviated with `~` for the user's home directory;
   - cumulative token stats `↑{in} ↓{out}` derived from `AgentEvent::AgentDone`
     usage, shown only once nonzero usage has been observed;
   - the model id retained.
2. A **startup welcome** is rendered as the first transcript item (a new
   `TranscriptItem::System` variant) containing a compact key hint line. It is
   transcript content, not a fixed UI row, so it scrolls away with history and
   does not shift the editor/footer row math.
3. A **key hint helper** module in `pi-coding-agent/interactive` formats a
   keybinding id into display text (e.g. `ctrl+c` -> `Ctrl+C`, `enter` ->
   `Enter`, `shift+enter` -> `Shift+Enter`) by reading
   `KeybindingsManager::get_keys(...)`. It is used by the welcome line. App-level
   actions with no registered binding (interrupt, expand) are rendered from a
   small static fallback table.
4. Tool result rows support an **expand/collapse** toggle. Collapsed keeps the
   current 3-line preview (default). Expanded raises the per-call preview cap to
   a larger bound (e.g. 20 lines). The toggle is triggered by `Ctrl+O` and
   applies globally to all tool rows, like the existing global `Ctrl+C` handling.
   The status words `running` / `done` / `error` are unchanged.

## 3. Non-goals

Deferred to later milestones:

- Per-tool custom renderers (diff view, bash framing, image output).
- Showing tool call arguments (requires extending `AgentEvent::ToolCallStart`
  with args in `pi-agent-core`; out of scope).
- ANSI color / theme system, context-window percentage, git branch, cost
  formatting, provider count.
- Configurable `app.*` keybinding actions for the new toggles (kept as direct
  `matches_key` checks, matching today's `Ctrl+C` approach).
- Slash commands, model/session selectors, overlays.

## 4. Key decisions

### 4.1 Footer stays one line; stats come from `AgentDone`

`AgentEvent::AgentDone { message: AssistantMessage }` already carries
`AssistantMessage.usage: Usage` with `input`, `output`, `cache_read`,
`cache_write`, `cost`. The event bridge will accumulate these into a running
total and emit a new `UiEvent::UsageUpdate { input, output }` (or a richer
struct). `InteractiveRoot` stores the latest totals and renders them in the
footer. This needs no change to `pi-agent-core`.

### 4.2 Welcome is transcript content, not a layout row

Adding a fixed header row above the transcript would shift the editor's row
index and break `scripted_interactive_keeps_prompt_anchored_below_transcript_viewport`
(which asserts `> typed` at row 4 for a height-6 terminal). Putting the welcome
inside the transcript viewport avoids that: the transcript viewport always shows
the bottom, so the welcome scrolls off as soon as there is enough output, and
the editor row position (driven by `viewport_height - editor_lines - 1`) is
unchanged.

A new `TranscriptItem::System { text: String }` variant is added. It renders as
a plain (width-safe) line, prefixed for readability, e.g. `  pi · Enter submit
· Shift+Enter newline · Ctrl+C interrupt/exit · Ctrl+O expand tools · PgUp/PgDn
scroll`. The transcript scroll bookkeeping already handles hidden-line growth,
so scrolled views stay stable.

### 4.3 Key hint helper is app-local

`pi-tui` already exposes `KeybindingsManager::get_keys(action)`. A tiny
`interactive/key_hints.rs` formats those key strings into display text and
resolves a few static app-level labels that have no registered binding. This
keeps `pi-tui` app-neutral (per the M6 decision 4.1) and avoids adding `app.*`
definitions to `TUI_KEYBINDINGS`.

### 4.4 Expand is a global, root-level toggle

Mirroring how `Ctrl+C` is handled in `InteractiveRoot::handle_input` (checked
before dispatching to the editor), `Ctrl+O` flips a `tool_output_expanded: bool`
on the root. `render_transcript_lines` gains a `max_tool_result_lines` parameter
(collapsed = 3, expanded = 20) so the existing unit test stays valid by passing
the collapsed value explicitly.

## 5. Architecture / changes

### 5.1 `pi-coding-agent/src/interactive/transcript.rs`

- Add `TranscriptItem::System { text: String }` with a constructor
  `TranscriptItem::system(text)`.
- `Transcript::new()` callers may seed a welcome item; or `InteractiveRoot::new`
  pushes it. (Chosen: `InteractiveRoot::new` pushes it once, so the transcript
  model stays pure.)
- `render_transcript_lines` and `render_tool_lines` gain a
  `max_tool_result_lines: usize` parameter; the truncation message uses that
  value. Existing callers pass `3` (collapsed) or the expanded cap.

### 5.2 `pi-coding-agent/src/interactive/event_bridge.rs`

- Add `UiEvent::UsageUpdate { input: u32, output: u32 }` (cache fields may be
  included later; minimal set is in/out).
- `InteractiveEventBridge` gains a running `Usage` accumulator and, on
  `AgentDone`, emits `UsageUpdate` with the cumulative totals. `AgentDone`
  still also emits `AssistantDone`.

### 5.3 `pi-coding-agent/src/interactive/key_hints.rs` (new)

- `pub fn format_key_text(keys: &[String]) -> String` — joins alternates with
  `/`, splits modifiers on `+`, capitalizes each part (`ctrl` -> `Ctrl`).
- `pub fn key_hint(kb: &KeybindingsManager, action: &str, description: &str) -> String`
  — returns `{keys} {description}` using `get_keys`; falls back to a static
  label table for app actions not in the keybinding manager (interrupt, expand,
  exit, scroll).
- Unit tests cover capitalization, alternates, and fallback.

### 5.4 `pi-coding-agent/src/interactive/app.rs`

- `InteractiveRoot` gains `usage: (u32, u32)` and
  `tool_output_expanded: bool`.
- `apply_events` handles `UiEvent::UsageUpdate` by storing the totals.
- `handle_input`: before editor dispatch, when idle, check
  `matches_key(event, "ctrl+o")` and flip `tool_output_expanded`, returning a
  render request. (Checked after the existing `Ctrl+C` block so priorities stay
  clear.)
- `render`: the welcome line is rendered via the transcript; the footer uses
  `format_footer` with `~`-abbreviated cwd and the usage stats. Tool rows use
  `render_transcript_lines(..., max_lines)` with `max_lines` = 3 or 20.
- `render_transcript_lines` / `render_transcript_viewport` pass the expanded cap
  through.

### 5.5 Footer format

Single line, keeping `status: <state>` first:

```
status: idle | ~/path | model-id | ↑1.2k ↓340
```

- cwd: replace home prefix with `~` using `std::env::var("HOME")`.
- stats: shown only when `usage != (0,0)`; formatted with a compact
  `format_tokens` helper (`<1000` -> raw, else `k`/`M`).
- width-safe via the existing `fit_line` helper (truncate from the right).

## 6. Error handling

No new error paths. Footer formatting is infallible (falls back to raw cwd when
`HOME` is unset). Usage accumulation is additive and saturates.

## 7. Testing strategy

Deterministic, offline, using the existing `VirtualTerminal` test harness and
faux providers.

- `interactive_transcript.rs`: `System` welcome item renders and scrolls;
  expanded tool rows show more lines; collapsed stays at 3 + truncation note.
- `interactive_event_bridge.rs`: `AgentDone` emits both `AssistantDone` and a
  cumulative `UsageUpdate` with the right totals across two turns.
- `key_hints` unit tests (in-module `#[cfg(test)]`): capitalization,
  alternates, fallback labels.
- `interactive_mode.rs`: scripted runs still contain `status: idle`, the welcome
  line on an empty transcript, and `↑/↓` stats after a faux turn; the
  height-6 anchor test still finds `> typed` at row 4.
- `interactive_abort.rs`: Ctrl+C behavior unchanged.

Manual smoke (tmux) remains the only non-CI check, unchanged from M6.

## 8. Rollout

Implement in this order, keeping `cargo test -p pi-tui` and
`cargo test -p pi-coding-agent` green between layers:

1. `key_hints` module + unit tests.
2. `TranscriptItem::System` + `max_tool_result_lines` threading + transcript
   tests.
3. `UiEvent::UsageUpdate` + bridge accumulator + bridge tests.
4. `InteractiveRoot` wiring: welcome item, footer formatting, expand toggle,
   usage render.
5. Update/extend scripted tests; run focused + workspace verification.

## 9. Risks

- The height-6 scripted anchor test is sensitive to row math. Keeping the
  welcome inside the transcript and the footer at one line avoids touching the
  layout formula.
- Usage totals are cumulative across the whole session; a reset on compaction is
  not required for M6 polish but should be noted (compaction events exist and
  could zero totals later if desired).
- Expanding tool output can exceed the transcript viewport; the viewport already
  truncates to the bottom rows, so expanded content simply scrolls, which is
  acceptable.
