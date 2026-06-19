# M11 Loader Components Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add reusable `Loader` and `CancellableLoader` components to `pi-tui`, matching the TypeScript loader component behavior closely enough for Rust interactive UI reuse.

**Architecture:** Keep loader rendering inside `pi-tui/src/components/loader.rs`. `Loader` owns message, indicator frames, and current frame; callers can advance the frame deterministically via `tick()` rather than embedding timers in the component. `CancellableLoader` wraps `Loader`, tracks an aborted flag, and invokes an optional cancellation callback when Escape/Ctrl+C matches the existing select cancel keybinding.

**Tech Stack:** Rust 2024, existing `pi-tui::Component`, `KeybindingsManager`, `TUI_KEYBINDINGS`, `truncate_to_width`, `visible_width`, and `StdinBuffer` test utilities.

---

## File Structure

- Create `crates/pi-tui/src/components/loader.rs`: `Loader`, `LoaderIndicatorOptions`, `CancellableLoader`, default frames, render/tick/message/cancel behavior.
- Modify `crates/pi-tui/src/components/mod.rs`: export loader types.
- Modify `crates/pi-tui/src/lib.rs`: re-export loader types from `components`.
- Create `crates/pi-tui/tests/loader.rs`: deterministic rendering, indicator customization, truncation, and cancellation tests.
- Modify `crates/pi-tui/tests/public_api.rs`: import loader public symbols to guard API exposure.
- Modify `docs/roadmap/M11-interactive-ux.md`: record loader component progress.

## Task 1: Loader Tests And Component

- [ ] **Step 1: Write failing loader tests**

Create `crates/pi-tui/tests/loader.rs`:

```rust
use std::cell::Cell;
use std::rc::Rc;

use pi_tui::{
    CancellableLoader, Component, KeybindingsManager, Loader, LoaderIndicatorOptions,
    StdinBuffer, TUI_KEYBINDINGS, visible_width,
};

fn feed(loader: &mut CancellableLoader, data: &str) {
    let mut buffer = StdinBuffer::new();
    for event in buffer.process(data) {
        loader.handle_input(&event);
    }
}

#[test]
fn loader_renders_message_with_default_spinner_and_padding() {
    let mut loader = Loader::new("Loading...");
    assert_eq!(loader.render(20), vec!["⠋ Loading...        "]);
}

#[test]
fn loader_tick_advances_indicator_frame() {
    let mut loader = Loader::new("Working");
    loader.tick();
    assert_eq!(loader.render(20), vec!["⠙ Working           "]);
}

#[test]
fn loader_supports_custom_indicator_and_message_updates() {
    let mut loader = Loader::new("Starting");
    loader.set_indicator(LoaderIndicatorOptions {
        frames: vec![".".to_string(), "o".to_string()],
    });
    loader.set_message("Running");
    assert_eq!(loader.render(12), vec![". Running   "]);
    loader.tick();
    assert_eq!(loader.render(12), vec!["o Running   "]);
}

#[test]
fn loader_can_hide_indicator() {
    let mut loader = Loader::new("Quiet");
    loader.set_indicator(LoaderIndicatorOptions { frames: Vec::new() });
    assert_eq!(loader.render(10), vec!["Quiet     "]);
}

#[test]
fn loader_truncates_to_render_width() {
    let mut loader = Loader::new("A very long loading message");
    let lines = loader.render(10);
    assert_eq!(lines.len(), 1);
    assert!(visible_width(&lines[0]) <= 10);
}

#[test]
fn cancellable_loader_sets_aborted_and_invokes_callback_on_escape() {
    let called = Rc::new(Cell::new(false));
    let called_for_callback = Rc::clone(&called);
    let mut loader = CancellableLoader::new(
        Loader::new("Working"),
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
    );
    loader.set_on_abort(Box::new(move || called_for_callback.set(true)));

    feed(&mut loader, "\x1b");

    assert!(loader.aborted());
    assert!(called.get());
}

#[test]
fn cancellable_loader_invokes_callback_only_once() {
    let count = Rc::new(Cell::new(0));
    let count_for_callback = Rc::clone(&count);
    let mut loader = CancellableLoader::new(
        Loader::new("Working"),
        KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
    );
    loader.set_on_abort(Box::new(move || count_for_callback.set(count_for_callback.get() + 1)));

    feed(&mut loader, "\x1b\x1b");

    assert!(loader.aborted());
    assert_eq!(count.get(), 1);
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test loader
```

Expected: compile failure because loader types are not defined/exported yet.

- [ ] **Step 3: Implement loader components**

Create `crates/pi-tui/src/components/loader.rs` with:

- `DEFAULT_LOADER_FRAMES: &[&str]` matching the TS braille frames.
- `LoaderIndicatorOptions { frames: Vec<String> }`.
- `Loader::new(message)`, `set_message`, `set_indicator`, `tick`, `render_text`.
- `Component for Loader`, rendering one padded/truncated line.
- `CancellableLoader::new(loader, keybindings)`, `aborted`, `set_on_abort`, `tick`, `loader_mut`.
- `Component for CancellableLoader`, delegating render and handling `tui.select.cancel`.

- [ ] **Step 4: Export loader types**

Update `crates/pi-tui/src/components/mod.rs`:

```rust
mod loader;
pub use loader::{CancellableLoader, Loader, LoaderIndicatorOptions};
```

Update `crates/pi-tui/src/lib.rs` component re-export list to include `CancellableLoader`, `Loader`, and `LoaderIndicatorOptions`.

- [ ] **Step 5: Run loader tests and verify they pass**

Run:

```bash
cargo test -p pi-tui --test loader
```

Expected: all loader tests pass.

## Task 2: Public API And Roadmap

- [ ] **Step 1: Add public API smoke import**

Update `crates/pi-tui/tests/public_api.rs` with:

```rust
let mut loader = Loader::new("Loading");
loader.tick();
let _ = CancellableLoader::new(
    loader,
    KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
);
```

and import `CancellableLoader`, `KeybindingsManager`, `Loader`, `TUI_KEYBINDINGS`.

- [ ] **Step 2: Update roadmap**

Under M11 item 3, add:

```markdown
> 进度：`pi-tui` 已新增可复用 `Loader` / `CancellableLoader`，支持 deterministic frame tick、消息更新、自定义/隐藏 indicator、宽度裁剪和 Escape/Ctrl+C 取消回调；coding-agent 侧硬编码 spinner 迁移仍待接入。
```

## Task 3: Verification And Commit

- [ ] **Step 1: Run verification**

Run:

```bash
cargo fmt --check
env -u NO_COLOR TERM=xterm-256color cargo test -p pi-tui
env -u NO_COLOR TERM=xterm-256color cargo test --workspace
cargo check --workspace
```

Expected: all pass. Use the color-enabled environment because existing markdown tests assert ANSI output.

- [ ] **Step 2: Commit**

Run:

```bash
git add crates/pi-tui/src/components/loader.rs crates/pi-tui/src/components/mod.rs crates/pi-tui/src/lib.rs crates/pi-tui/tests/loader.rs crates/pi-tui/tests/public_api.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-loader-components.md
git commit -m "feat: add tui loader components"
```

Expected: commit succeeds with only loader-slice files included.
