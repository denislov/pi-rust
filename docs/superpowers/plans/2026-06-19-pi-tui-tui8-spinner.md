# TUI-8 Spinner/Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not commit unless the user explicitly requests a commit.

**Goal:** Add an animated braille-dot spinner to the footer status field when the agent is running, driven by a 120ms timer in the render loop.

**Architecture:** Single-layer change entirely in `pi-coding-agent/src/interactive/app.rs`. `InteractiveRoot` gains a `spinner_frame: usize` field. The `footer()` method renders `"{spinner_char} running"` when `status == Running`. The render loop's Running branch gains a `tokio::time::sleep(SPINNER_INTERVAL)` select arm that advances the frame and requests a force render. `spinner_frame` is part of `InteractiveRenderState` so differential rendering detects frame changes. `set_status(Idle)` resets the frame to 0.

**Tech Stack:** Rust edition 2024; existing `pi-coding-agent` interactive loop (`tokio::select!`, `RenderScheduler`, `InteractiveRoot`), existing `paint_with` from TUI-8 first slice.

## Global Constraints

- The spinner only appears in the footer when `status == Running`; idle footer is completely unchanged (existing `status: idle` substring assertions must still pass).
- The 120ms timer only runs when `running.is_some()`; idle periods must not wake the render loop.
- `spinner_frame` must be part of `InteractiveRenderState` so frame changes trigger differential redraw.
- `set_status(Idle)` resets `spinner_frame` to 0.
- Tests are deterministic and offline; no real provider key, no network, no real TTY. In-file unit tests construct `InteractiveRoot` directly and assert substring presence (not exact bytes) to avoid `color_enabled()` cache dependence.
- Run checks from `pi-rust/` (the Cargo workspace root): `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo test --workspace`, `cargo check --workspace`.

## Reference: existing signatures the plan builds on

These already exist; do not re-implement them:

```rust
// crates/pi-coding-agent/src/interactive/app.rs (current)
const NORMAL_RENDER_INTERVAL: Duration = Duration::from_millis(16);
const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;

struct InteractiveRoot {
    transcript: Transcript,
    editor: Editor,
    submitted: Arc<Mutex<Option<String>>>,
    scroll_command: Arc<Mutex<Option<TranscriptScrollCommand>>>,
    pending_submit: Option<String>,
    action: InteractiveAction,
    status: InteractiveStatus,
    viewport_width: usize,
    viewport_height: usize,
    cwd: PathBuf,
    model_id: String,
    session_label: String,
    usage: (u32, u32),
    tool_output_expanded: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct InteractiveRenderState {
    editor_text: String,
    editor_cursor: usize,
    transcript: Vec<TranscriptItem>,
    transcript_scroll_offset: usize,
    transcript_has_new_output_below: bool,
    status: InteractiveStatus,
    tool_output_expanded: bool,
}
```

The `footer()` method currently produces `paint_with(&format!("status: {status_str}"), &status_style, color)` where `status_str` is `"idle"` or `"running"`.

The `set_status()` method is currently just `self.status = status;`.

The `render_state()` method ends with `tool_output_expanded: self.tool_output_expanded,`.

The render loop's Running branch (`run_started_interactive_loop`, inside `if let Some(mut task) = running.take()`) has a `tokio::select!` with four existing branches: `sleep_render_delay`, `input.recv()`, `task.events.recv()`, and `task.done`. The spinner timer branch is added as a fifth branch.

## File Structure

- Modify: `crates/pi-coding-agent/src/interactive/app.rs` — add `SPINNER_INTERVAL`/`SPINNER_FRAMES` constants, `InteractiveRoot.spinner_frame` field, `footer()` spinner logic, `render_state()` field, `set_status()` reset, render loop timer branch, in-file tests.

No new files, no changes to `pi-tui` or other crates.

---

## Task 1: Spinner — constants, field, footer rendering, render_state, set_status reset

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

**Interfaces:**
- Consumes: `pi_tui::{paint_with, STATUS_IDLE, STATUS_RUNNING, color_enabled}` (already imported from TUI-8 first slice).
- Produces: `InteractiveRoot.spinner_frame` field, `SPINNER_INTERVAL`/`SPINNER_FRAMES` constants, `footer()` renders spinner when Running, `render_state()` includes `spinner_frame`, `set_status(Idle)` resets frame.

- [ ] **Step 1: Write failing tests**

In `crates/pi-coding-agent/src/interactive/app.rs`, the existing `#[cfg(test)] mod tests` block (near the end of the file) contains `render_transcript_lines_compacts_tool_rows_and_truncates_noisy_output`, `render_transcript_lines_colors_error_item_red_bold`, and `ctrl_o_toggles_tool_output_expansion_in_root`. Append these new tests inside the same `mod tests` block (after `ctrl_o_toggles_tool_output_expansion_in_root`):

```rust
    #[test]
    fn footer_shows_spinner_when_running() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        let footer = root.footer();
        assert!(
            footer.contains("running"),
            "footer should contain 'running' when status is Running: {footer}"
        );
        let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .any(|frame| footer.contains(frame));
        assert!(
            has_spinner,
            "footer should contain a braille spinner char when Running: {footer}"
        );
    }

    #[test]
    fn footer_no_spinner_when_idle() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Idle);
        let footer = root.footer();
        assert!(
            footer.contains("status: idle"),
            "footer should contain 'status: idle' when Idle: {footer}"
        );
        let has_spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
            .iter()
            .any(|frame| footer.contains(frame));
        assert!(
            !has_spinner,
            "footer should NOT contain a braille spinner char when Idle: {footer}"
        );
    }

    #[test]
    fn spinner_frame_advances_through_sequence() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);

        root.spinner_frame = 3;
        let footer_at_3 = root.footer();
        assert!(
            footer_at_3.contains("⠼"),
            "footer at frame 3 should contain '⠼': {footer_at_3}"
        );

        root.spinner_frame = 4;
        let footer_at_4 = root.footer();
        assert!(
            footer_at_4.contains("⠴"),
            "footer at frame 4 should contain '⠴': {footer_at_4}"
        );
    }

    #[test]
    fn set_status_idle_resets_spinner_frame() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.spinner_frame = 5;
        root.set_status(InteractiveStatus::Idle);
        assert_eq!(
            root.spinner_frame, 0,
            "set_status(Idle) should reset spinner_frame to 0"
        );
    }

    #[test]
    fn render_state_changes_with_spinner_frame() {
        let mut root = InteractiveRoot::new(
            PathBuf::from("."),
            "faux-model".to_string(),
            "no-session".to_string(),
        );
        root.set_status(InteractiveStatus::Running);
        root.spinner_frame = 0;
        let state_at_0 = root.render_state();
        root.spinner_frame = 1;
        let state_at_1 = root.render_state();
        assert_ne!(
            state_at_0, state_at_1,
            "render_state should differ when spinner_frame changes"
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib footer_shows_spinner_when_running footer_no_spinner_when_idle spinner_frame_advances_through_sequence set_status_idle_resets_spinner_frame render_state_changes_with_spinner_frame
```

Expected: compile errors — `spinner_frame` field does not exist on `InteractiveRoot`; `footer()` does not render a spinner when Running; `set_status` does not reset `spinner_frame`; `InteractiveRenderState` has no `spinner_frame` field.

- [ ] **Step 3: Add spinner constants**

In `crates/pi-coding-agent/src/interactive/app.rs`, the constants block near line 64-67 is:

```rust
static INTERACTIVE_ID: AtomicUsize = AtomicUsize::new(1);
const NORMAL_RENDER_INTERVAL: Duration = Duration::from_millis(16);
const MAX_TOOL_RESULT_LINES: usize = 3;
const EXPANDED_TOOL_RESULT_LINES: usize = 20;
```

Add two new constants after `EXPANDED_TOOL_RESULT_LINES`:

```rust
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
```

- [ ] **Step 4: Add `spinner_frame` field to `InteractiveRoot`**

The current `struct InteractiveRoot` (lines 149-164) ends with:

```rust
    usage: (u32, u32),
    tool_output_expanded: bool,
}
```

Add `spinner_frame` after `tool_output_expanded`:

```rust
    usage: (u32, u32),
    tool_output_expanded: bool,
    spinner_frame: usize,
}
```

- [ ] **Step 5: Initialize `spinner_frame` in `new()`**

The current `InteractiveRoot::new` constructor (lines 177-219) ends with:

```rust
            usage: (0, 0),
            tool_output_expanded: false,
        }
    }
```

Add `spinner_frame: 0`:

```rust
            usage: (0, 0),
            tool_output_expanded: false,
            spinner_frame: 0,
        }
    }
```

- [ ] **Step 6: Add `spinner_frame` to `InteractiveRenderState`**

The current `InteractiveRenderState` struct (lines 166-175) ends with:

```rust
    status: InteractiveStatus,
    tool_output_expanded: bool,
}
```

Add `spinner_frame`:

```rust
    status: InteractiveStatus,
    tool_output_expanded: bool,
    spinner_frame: usize,
}
```

- [ ] **Step 7: Update `render_state()` to include `spinner_frame`**

The current `render_state()` method (lines 312-322) ends with:

```rust
            status: self.status,
            tool_output_expanded: self.tool_output_expanded,
        }
    }
```

Add `spinner_frame`:

```rust
            status: self.status,
            tool_output_expanded: self.tool_output_expanded,
            spinner_frame: self.spinner_frame,
        }
    }
```

- [ ] **Step 8: Update `set_status()` to reset frame on Idle**

The current `set_status` method (lines 277-279) is:

```rust
    fn set_status(&mut self, status: InteractiveStatus) {
        self.status = status;
    }
```

Replace with:

```rust
    fn set_status(&mut self, status: InteractiveStatus) {
        if status == InteractiveStatus::Idle {
            self.spinner_frame = 0;
        }
        self.status = status;
    }
```

- [ ] **Step 9: Update `footer()` to render spinner when Running**

The current `footer()` method (lines 281-310) has this status_str logic:

```rust
        let status_str = match self.status {
            InteractiveStatus::Idle => "idle",
            InteractiveStatus::Running => "running",
        };
```

Replace with:

```rust
        let status_str = match self.status {
            InteractiveStatus::Idle => "idle".to_string(),
            InteractiveStatus::Running => {
                let spinner = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
                format!("{spinner} running")
            }
        };
```

Note: `status_str` changes from `&str` to `String`. The existing `paint_with(&format!("status: {status_str}"), &status_style, color)` call already uses `format!` which accepts `String` arguments, so no further change is needed.

- [ ] **Step 10: Run tests to verify they pass**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent --lib footer_shows_spinner_when_running footer_no_spinner_when_idle spinner_frame_advances_through_sequence set_status_idle_resets_spinner_frame render_state_changes_with_spinner_frame
```

Expected: PASS (5 tests).

- [ ] **Step 11: Run the full pi-coding-agent suite to check for regressions**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS (all existing tests green; `status: idle` substring assertions still hold because Idle footer is unchanged).

- [ ] **Step 12: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs
git commit -m "feat(interactive): add braille spinner to footer when agent is running"
```

---

## Task 2: Render loop — spinner timer branch

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/app.rs`

**Interfaces:**
- Consumes: `SPINNER_INTERVAL`, `SPINNER_FRAMES`, `InteractiveRoot.spinner_frame` (from Task 1).
- Produces: the Running branch of `run_started_interactive_loop` gains a `tokio::time::sleep(SPINNER_INTERVAL)` select arm that advances the frame and requests a force render.

- [ ] **Step 1: Add the spinner timer branch to the Running select**

In `crates/pi-coding-agent/src/interactive/app.rs`, the `run_started_interactive_loop` function has a `tokio::select!` inside the `if let Some(mut task) = running.take()` block (lines 529-587). The current select has four branches. The third branch ends and the fourth (`done`) branch begins like this:

```rust
                    running = Some(task);
                }
                done = &mut task.done => {
```

Insert a new fifth branch between the `maybe_event` branch's closing `}` and the `done = &mut task.done =>` line:

```rust
                    running = Some(task);
                }
                _ = tokio::time::sleep(SPINNER_INTERVAL) => {
                    if let Some(root) = tui.component_as_mut::<InteractiveRoot>(root_id) {
                        root.spinner_frame =
                            (root.spinner_frame + 1) % SPINNER_FRAMES.len();
                    }
                    render_scheduler.request(true);
                    running = Some(task);
                }
                done = &mut task.done => {
```

The full select block after insertion will have five branches in this order: `sleep_render_delay`, `input.recv()`, `task.events.recv()`, `tokio::time::sleep(SPINNER_INTERVAL)`, `task.done`.

Key points:
- `tokio::time::sleep(SPINNER_INTERVAL)` creates a fresh future each select iteration, firing 120ms after the last select iteration. This gives roughly 120ms cadence with slight jitter from other branches — acceptable for a spinner.
- After the timer fires: advance the frame, `request(true)` for a force render (bypass 16ms throttle), and put `task` back into `running`.
- No `if` guard is needed on this branch because the entire `tokio::select!` is already inside the `if let Some(task) = running.take()` block — it only runs when a prompt task is active.

- [ ] **Step 2: Verify the workspace builds**

Run from `pi-rust/`:

```bash
cargo check -p pi-coding-agent
```

Expected: PASS (no errors). If there is a borrow conflict because `task` is moved into `running = Some(task)` in the new branch, ensure the branch body puts `task` back before the branch ends (the `running = Some(task);` line handles this).

- [ ] **Step 3: Run the full pi-coding-agent suite**

Run from `pi-rust/`:

```bash
cargo test -p pi-coding-agent
```

Expected: PASS (all existing tests green). The spinner timer branch is not exercised by scripted tests (they complete prompts too fast for 120ms to fire), but it must not break compilation or existing test behavior.

- [ ] **Step 4: Commit**

```bash
cd pi-rust
git add crates/pi-coding-agent/src/interactive/app.rs
git commit -m "feat(interactive): drive footer spinner with 120ms timer in render loop"
```

---

## Task 3: Final verification

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
git log --oneline -5
```

Expected: the three commits sit on top of the spec commit (`6017c54 docs: add TUI-8 spinner/progress design`), with clean, focused messages:
1. `feat(interactive): add braille spinner to footer when agent is running`
2. `feat(interactive): drive footer spinner with 120ms timer in render loop`
3. (no commit for Task 3 unless a fix was needed)
