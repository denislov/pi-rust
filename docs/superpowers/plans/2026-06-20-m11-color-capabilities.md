# M11 Color Capabilities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `pi-tui` styling beyond 8 ANSI colors with 256-color/RGB SGR output and deterministic terminal color capability detection.

**Architecture:** Keep the implementation in `crates/pi-tui/src/style.rs` because all current public color/style APIs live there. Add non-breaking `Color::Ansi256` and `Color::Rgb` variants, a `ColorLevel` capability enum, injectable environment detection for tests, and a `paint_with_level` helper while preserving existing `paint_with(text, style, enabled)` behavior.

**Tech Stack:** Rust 2024, existing `Style`, `Color`, `paint`, `paint_with`, and offline unit tests.

---

## File Structure

- Modify `crates/pi-tui/src/style.rs`: add expanded color variants, SGR parameter generation, `ColorLevel`, `detect_color_level_from_env`, `color_level`, and `paint_with_level`.
- Modify `crates/pi-tui/src/lib.rs`: re-export `ColorLevel`, `color_level`, `detect_color_level_from_env`, and `paint_with_level`.
- Modify `crates/pi-tui/tests/style.rs`: add tests for 256/RGB foreground/background output and capability detection.
- Modify `crates/pi-tui/tests/public_api.rs`: smoke import/use the new public symbols.
- Modify `docs/roadmap/M11-interactive-ux.md`: record theme foundation progress.

## Task 1: Tests First

- [ ] **Step 1: Write failing style tests**

Add tests in `crates/pi-tui/tests/style.rs` for:

```rust
#[test]
fn paint_with_enabled_ansi256_fg_and_bg() {
    let mut style = Style::fg(Color::Ansi256(202));
    style.bg = Color::Ansi256(17);
    assert_eq!(
        paint_with("hi", &style, true),
        "\x1b[38;5;202;48;5;17mhi\x1b[0m"
    );
}

#[test]
fn paint_with_enabled_rgb_fg_and_bg() {
    let mut style = Style::fg(Color::Rgb(1, 2, 3));
    style.bg = Color::Rgb(4, 5, 6);
    assert_eq!(
        paint_with("hi", &style, true),
        "\x1b[38;2;1;2;3;48;2;4;5;6mhi\x1b[0m"
    );
}

#[test]
fn paint_with_level_downgrades_when_color_disabled() {
    let style = Style::fg(Color::Rgb(1, 2, 3)).bold();
    assert_eq!(paint_with_level("hi", &style, ColorLevel::None), "hi");
}

#[test]
fn detect_color_level_honors_no_color_and_dumb() {
    assert_eq!(
        detect_color_level_from_env([("NO_COLOR", "1"), ("TERM", "xterm-256color")]),
        ColorLevel::None
    );
    assert_eq!(
        detect_color_level_from_env([("TERM", "dumb")]),
        ColorLevel::None
    );
}

#[test]
fn detect_color_level_detects_truecolor_and_ansi256() {
    assert_eq!(
        detect_color_level_from_env([("COLORTERM", "truecolor"), ("TERM", "xterm-256color")]),
        ColorLevel::TrueColor
    );
    assert_eq!(
        detect_color_level_from_env([("TERM", "screen-256color")]),
        ColorLevel::Ansi256
    );
    assert_eq!(detect_color_level_from_env([("TERM", "xterm")]), ColorLevel::Ansi16);
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test style
```

Expected: compile failure because `Color::Ansi256`, `Color::Rgb`, `ColorLevel`, `paint_with_level`, and `detect_color_level_from_env` do not exist yet.

## Task 2: Implementation

- [ ] **Step 1: Implement color variants and SGR parameters**

Update `crates/pi-tui/src/style.rs`:

- Add `Color::Ansi256(u8)` and `Color::Rgb(u8, u8, u8)`.
- Replace fixed `fg_code/bg_code` helpers with `sgr_params(self, foreground: bool) -> Vec<String>`.
- Preserve existing ANSI16 output for current variants, so existing tests keep passing.

- [ ] **Step 2: Add capability detection and level-aware paint**

Add:

- `ColorLevel::{None, Ansi16, Ansi256, TrueColor}`.
- `detect_color_level_from_env<I, K, V>(env: I) -> ColorLevel` accepting iterable key/value pairs.
- `color_level()` cached from real environment.
- `color_enabled()` implemented as `color_level() != ColorLevel::None`.
- `paint_with_level(text, style, level)` returning plain text for `None`, otherwise emitting SGR. This slice does not downsample RGB/256 to lower color levels; it preserves explicit styles when any color support is enabled.

- [ ] **Step 3: Export public symbols and public API smoke**

Update `crates/pi-tui/src/lib.rs` style re-exports.

Update `crates/pi-tui/tests/public_api.rs` to import/use:

```rust
let _ = paint_with_level("x", &Style::fg(Color::Ansi256(1)), ColorLevel::Ansi256);
let _ = detect_color_level_from_env([("TERM", "xterm-256color")]);
let _ = color_level();
```

- [ ] **Step 4: Update roadmap**

Under M11 item 5, add:

```markdown
> 进度：`pi-tui` style 已支持 ANSI 256/RGB SGR 输出与可注入的颜色能力探测（NO_COLOR、dumb、truecolor/24bit、*-256color）；dark/light/custom theme 对象和组件 theme 参数仍待接入。
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

Expected: all pass.

- [ ] **Step 2: Commit**

Run:

```bash
git add crates/pi-tui/src/style.rs crates/pi-tui/src/lib.rs crates/pi-tui/tests/style.rs crates/pi-tui/tests/public_api.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-color-capabilities.md
git commit -m "feat: add tui color capability detection"
```
