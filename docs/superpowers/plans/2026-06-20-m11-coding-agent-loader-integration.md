# M11 Coding Agent Loader Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Rust coding-agent interactive footer's local hard-coded spinner frames with the reusable `pi_tui::Loader`.

**Architecture:** Keep `InteractiveRoot`'s `spinner_frame` render-state counter stable for existing render diff tests, but make footer status text derive from a helper that creates/ticks `pi_tui::Loader`. Change the runtime spinner tick to `wrapping_add(1)` so `pi-coding-agent` no longer needs to know the loader frame count.

**Tech Stack:** Rust 2024, existing `pi-coding-agent` interactive tests, `pi_tui::Loader`.

---

## File Structure

- Modify `crates/pi-coding-agent/src/interactive/app.rs`: import `Loader`, remove local spinner frames, add `running_status_text(frame)`, use it in `footer`, and increment spinner frame without local modulo.
- Modify `docs/roadmap/M11-interactive-ux.md`: record coding-agent footer spinner migration.

## Task 1: Tests First

- [ ] **Step 1: Add failing helper test**

Add this test near the existing spinner footer tests in `crates/pi-coding-agent/src/interactive/app.rs`:

```rust
#[test]
fn running_status_text_uses_loader_sequence() {
    assert_eq!(running_status_text(0), "⠋ running");
    assert_eq!(running_status_text(1), "⠙ running");
}
```

- [ ] **Step 2: Run test and verify it fails**

Run:

```bash
env -u NO_COLOR TERM=xterm-256color cargo test -p pi-coding-agent interactive::app::tests::running_status_text_uses_loader_sequence
```

Expected: compile failure because `running_status_text` does not exist.

## Task 2: Implementation

- [ ] **Step 1: Use `pi_tui::Loader` for running footer text**

Update `crates/pi-coding-agent/src/interactive/app.rs`:

- Import `Loader`.
- Remove `SPINNER_FRAMES`.
- Add:

```rust
fn running_status_text(frame: usize) -> String {
    let mut loader = Loader::new("running");
    for _ in 0..frame {
        loader.tick();
    }
    loader.render_text()
}
```

- In `InteractiveRoot::footer`, replace local spinner formatting with `running_status_text(self.spinner_frame)`.
- In the event loop spinner interval branch, replace modulo increment with `root.spinner_frame = root.spinner_frame.wrapping_add(1);`.

- [ ] **Step 2: Update roadmap**

Under M11 item 3, add:

```markdown
> 进度：`pi-coding-agent` interactive footer spinner 已迁移为复用 `pi_tui::Loader`，不再在 coding-agent 侧维护独立 spinner frame 表。
```

## Task 3: Verification And Commit

- [ ] **Step 1: Run verification**

Run:

```bash
cargo fmt --check
env -u NO_COLOR TERM=xterm-256color cargo test -p pi-coding-agent
env -u NO_COLOR TERM=xterm-256color cargo test --workspace
cargo check --workspace
```

Expected: all pass.

- [ ] **Step 2: Commit**

Run:

```bash
git add crates/pi-coding-agent/src/interactive/app.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-coding-agent-loader-integration.md
git commit -m "feat: reuse tui loader in interactive footer"
```
