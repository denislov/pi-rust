# Design: pi-tui TUI-8 spinner/progress for running agent

- Date: 2026-06-19
- Status: Draft (pending review)
- Scope: Second slice of TUI-8 from `docs/TUI_INTERACTION_ROADMAP.md` — animated spinner in the footer status field when the agent is running.
- Depends on: `pi-coding-agent` interactive loop (`RenderScheduler`, `InteractiveRoot`, `footer`, `render_state`), TUI-8 first slice (color + Markdown polish, completed).

## 1. Context

The TUI-8 first slice (semantic 8-color styling + Markdown polish) is complete. The interactive transcript now has visual hierarchy, but when the agent is running there is no dynamic feedback — the footer shows a static `status: running` string with no animation. Users get no visual signal that the agent is actively working, especially during silent periods between assistant deltas.

This spec adds a compact braille spinner to the footer status field, animated at 120ms per frame, only while `status == Running`. The spinner is driven by a dedicated timer branch in the `tokio::select!` render loop, active only when a prompt task is running.

Behavioral reference (TS): `pi/packages/tui/src/components/` and `pi/packages/coding-agent/src/` — the TS side uses spinners in the status bar. This Rust slice uses a braille-dot spinner matching pi/Codex conventions.

## 2. Goals and success criteria

Add an animated spinner to the footer status field when the agent is running, driven by a 120ms timer in the render loop.

Done when:

1. `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, and `cargo check --workspace` pass from `pi-rust/`.
2. When `status == Running`, the footer status field displays `"{spinner_char} running"` where `spinner_char` is a braille dot from the 10-frame sequence, advancing every 120ms.
3. When `status == Idle`, the footer status field displays `"status: idle"` with no spinner — identical to current behavior.
4. `set_status(Idle)` resets `spinner_frame` to 0, so the next Running period starts from the first frame.
5. `spinner_frame` is part of `InteractiveRenderState`, so frame changes trigger differential redraw.
6. The 120ms timer only runs when `running.is_some()`; idle periods do not wake the render loop.
7. Existing substring assertions (`status: idle`, `status: running` — where present) continue to pass.
8. The offline test suite passes with no network access and no credentials.

## 3. Non-goals (this increment)

- Per-tool spinners in transcript rows (each tool row getting its own spinner).
- Progress bars or percentage indicators.
- Spinner for streaming assistant text (only agent running state triggers the spinner).
- Configurable spinner style or speed.
- Changes to `pi-tui` core (`RenderScheduler`, `Terminal`, `Tui`).
- Changes to `pi-agent-core` or `pi-ai`.

## 4. Design

### 4.1 Architecture and data flow

Single-layer change, entirely in `crates/pi-coding-agent/src/interactive/app.rs`:

    Render loop (run_started_interactive_loop, Running branch)
      |
      +- Running: tokio::select! gains SPINNER_TICK branch
      |   -> sleep(120ms) -> root.spinner_frame = (frame+1) % 10 -> request force render
      |
      +- InteractiveRoot::footer()
          +- status == Running -> "{spinner_char} running" (replaces "running")
          +- status == Idle    -> "idle" (unchanged)

**Data flow:** 120ms timer fires -> advance `spinner_frame` -> `RenderRequest::FORCE` -> `flush_render_if_sensitive` -> `InteractiveRoot::render` -> `footer()` reads `spinner_frame` to pick the current braille char -> differential render detects the footer line changed -> write to terminal.

**Key invariants:**

- The spinner only appears in the footer when `status == Running`; idle footer is completely unchanged (including existing `status: idle` substring assertions).
- `spinner_frame` is part of `InteractiveRenderState` so differential rendering detects frame changes and triggers repaint.
- The timer only exists when `running.is_some()`; idle has no extra wakeup, wasting no CPU.
- Braille characters are width-1 (`visible_width` handles them correctly), so footer `fit_line` truncation is safe.

### 4.2 Braille spinner constants

At the top of `app.rs`, near `NORMAL_RENDER_INTERVAL`:

```rust
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
```

10 frames, cycling. `spinner_frame % SPINNER_FRAMES.len()` indexes the array.

### 4.3 InteractiveRoot: new field and initialization

Add `spinner_frame: usize` to `struct InteractiveRoot` (after `tool_output_expanded: bool`).

In `InteractiveRoot::new`, initialize `spinner_frame: 0`.

### 4.4 footer() spinner rendering

The current footer status logic (after TUI-8 first slice) is:

```rust
let status_str = match self.status {
    InteractiveStatus::Idle => "idle",
    InteractiveStatus::Running => "running",
};
// ...
paint_with(&format!("status: {status_str}"), &status_style, color),
```

Change the `Running` arm to produce a spinner-prefixed string:

```rust
let status_str = match self.status {
    InteractiveStatus::Idle => "idle".to_string(),
    InteractiveStatus::Running => {
        let spinner = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
        format!("{spinner} running")
    }
};
// ... (status_style unchanged, still STATUS_RUNNING)
paint_with(&format!("status: {status_str}"), &status_style, color),
```

Note: `status_str` becomes `String` (was `&str`) because the Running arm now allocates. This is fine — footer is called once per render.

### 4.5 render_state() — include spinner_frame

`InteractiveRenderState` must include `spinner_frame` so differential rendering detects frame changes. Add the field after `tool_output_expanded`:

```rust
#[derive(Debug, Clone, PartialEq)]
struct InteractiveRenderState {
    // ... existing fields ...
    tool_output_expanded: bool,
    spinner_frame: usize,
}
```

And in `render_state()`:

```rust
fn render_state(&self) -> InteractiveRenderState {
    InteractiveRenderState {
        // ... existing fields ...
        tool_output_expanded: self.tool_output_expanded,
        spinner_frame: self.spinner_frame,
    }
}
```

### 4.6 set_status() — reset frame on Idle

When the agent transitions to Idle (prompt finished), reset `spinner_frame` so the next Running period starts from frame 0:

```rust
fn set_status(&mut self, status: InteractiveStatus) {
    if status == InteractiveStatus::Idle {
        self.spinner_frame = 0;
    }
    self.status = status;
}
```

### 4.7 Render loop — Running branch timer

In `run_started_interactive_loop`, the `running.is_some()` branch has a `tokio::select!` with three existing branches (`sleep_render_delay`, `input.recv()`, `task.events.recv()`). Add a fourth branch for the spinner tick:

```rust
_ = tokio::time::sleep(SPINNER_INTERVAL), if task_is_running => {
    if let Some(root) = tui.component_as_mut::<InteractiveRoot>(root_id) {
        root.spinner_frame = (root.spinner_frame + 1) % SPINNER_FRAMES.len();
    }
    render_scheduler.request(true);
    running = Some(task);
}
```

Where `task_is_running` is a guard that keeps the branch enabled. Since this code is inside the `if let Some(mut task) = running.take()` block, `task` is available and `running` is `None` at this point. The guard should be `true` (always enabled in this branch) or a local bool set to `true` before the select. The simplest form:

```rust
_ = tokio::time::sleep(SPINNER_INTERVAL) => {
    if let Some(root) = tui.component_as_mut::<InteractiveRoot>(root_id) {
        root.spinner_frame = (root.spinner_frame + 1) % SPINNER_FRAMES.len();
    }
    render_scheduler.request(true);
    running = Some(task);
}
```

No guard needed — this branch is only reached when `running.is_some()` (we're inside the `if let Some(task) = running.take()` block). The `tokio::time::sleep` future is created fresh each iteration of the select loop, so it fires 120ms after the last select iteration, not 120ms after the last spinner tick. This is acceptable — the spinner advances at roughly 120ms cadence, with slight jitter from other select branches. If jitter is visible, a dedicated `tokio::time::interval` can be used instead, but the simple sleep is sufficient for this slice.

After the timer fires: advance the frame, request a force render (bypass 16ms throttle), and put `task` back into `running`.

### 4.8 Existing assertion compatibility

- `interactive_abort.rs` asserts `output.contains("status: idle")` after abort — Idle footer is unchanged. ✅
- `interactive_mode.rs` `scripted_interactive_footer_shows_usage_after_a_turn` asserts `status: idle` — turn ends in Idle. ✅
- No existing test asserts the exact `"status: running"` substring during Running state (tests check post-turn Idle state). ✅
- The `ctrl_o_toggles_tool_output_expansion_in_root` test uses `InteractiveRoot::new` and asserts on transcript content, not footer running state. ✅

### 4.9 Error handling

No new error types. Spinner frame advance is `usize` modulo 10 — no overflow. `SPINNER_FRAMES` is a static array, index is always valid (`% len()`). The `component_as_mut` returns `Option`; `if let Some` handles the impossible-missing-root case by silently skipping the frame advance (no panic).

### 4.10 Testing strategy

All tests are offline and deterministic.

**In-file unit tests** (`app.rs` `#[cfg(test)] mod tests`):

- `footer_shows_spinner_when_running`: construct `InteractiveRoot`, `set_status(Running)`, assert `footer()` contains a braille char from `SPINNER_FRAMES` and contains `"running"`.
- `footer_no_spinner_when_idle`: `set_status(Idle)`, assert `footer()` equals (or contains) `"status: idle"` and does NOT contain any `SPINNER_FRAMES` char.
- `spinner_frame_advances`: set `spinner_frame` to 3, assert `footer()` contains `SPINNER_FRAMES[3]`; advance to 4, assert contains `SPINNER_FRAMES[4]`.
- `set_status_idle_resets_spinner_frame`: set `spinner_frame` to 5, `set_status(Idle)`, assert `spinner_frame == 0`.
- `render_state_changes_with_spinner_frame`: assert `render_state()` with `spinner_frame=0` != `render_state()` with `spinner_frame=1`.

These tests construct `InteractiveRoot` directly and call `footer()`/`render_state()` — no TTY, no timer, no `color_enabled()` dependency (footer uses `paint_with` which is deterministic given the `color` flag; but `footer()` calls `color_enabled()` internally — tests should only assert substring presence, not exact bytes, to avoid `color_enabled()` cache dependence).

**Scripted tests** (`interactive_mode.rs`):

- No new scripted tests — the 120ms timer is hard to control deterministically in the scripted harness. Existing tests continue to pass (they assert Idle state).
- Optional: a scripted test that runs a prompt to completion and asserts the final frame does NOT contain a spinner char (since status is Idle). This is already covered by existing `status: idle` assertions.

### 4.11 File structure

| File | Operation |
|---|---|
| `pi-coding-agent/src/interactive/app.rs` | edit: add `SPINNER_INTERVAL`/`SPINNER_FRAMES` constants, `InteractiveRoot.spinner_frame` field, `footer()` spinner logic, `render_state()` field, `set_status()` reset, render loop timer branch, in-file tests |

No new files, no changes to `pi-tui` or other crates.

### 4.12 Verification

Run from `pi-rust/`:

```bash
cargo fmt --check
cargo test -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

All must pass.

## 5. Key decisions and constraints

- **Footer status field** for spinner location — minimal intrusion, no transcript layout change.
- **120ms per frame** — visually smooth (~8fps), not flashy, matches common terminal tools.
- **Braille dot sequence** (10 frames) — modern, compact (1 column), widely supported, matches pi/Codex.
- **Running-only timer** — 120ms `tokio::time::sleep` branch in the Running `select!` block; idle has no wakeup, wasting no CPU.
- **`spinner_frame` in `InteractiveRenderState`** — differential rendering detects frame changes and repaints.
- **`set_status(Idle)` resets frame** — next Running period starts from frame 0.
- **No `pi-tui` core changes** — entirely in `pi-coding-agent`, preserving the "pi-tui is app-neutral" boundary.
- **Substring assertions stay green** — no existing test asserts exact `"status: running"`; Idle assertions are unaffected.
