# M11 SettingsList Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable `SettingsList` component to `pi-tui` for selector/dialog settings screens, covering rendering, navigation, search, value cycling, and callbacks.

**Architecture:** Implement the first Rust `SettingsList` slice in `crates/pi-tui/src/components/settings_list.rs`, following the existing `SelectList` keybinding and fuzzy-filtering patterns. Keep the API data-first and deterministic: `SettingItem` describes labels/current values/cycle values, `SettingsListOptions` toggles search, and callbacks report value changes or cancellation. Defer TS submenu factories to a later selector/dialog slice because Rust does not yet have the selector stack those submenus should host.

**Tech Stack:** Rust 2024, existing `pi-tui::Component`, `InputEvent`, `KeybindingsManager`, `TUI_KEYBINDINGS`, `fuzzy_filter_indices`, `truncate_to_width`, `visible_width`, and `StdinBuffer` tests.

---

## File Structure

- Create `crates/pi-tui/src/components/settings_list.rs`: `SettingItem`, `SettingsListOptions`, `SettingsList`, render/navigation/search/value-cycle/callback behavior.
- Modify `crates/pi-tui/src/components/mod.rs`: export settings list types.
- Modify `crates/pi-tui/src/lib.rs`: re-export settings list types from the crate root.
- Create `crates/pi-tui/tests/settings_list.rs`: focused component tests.
- Modify `crates/pi-tui/tests/public_api.rs`: smoke import and instantiate settings list symbols.
- Modify `docs/roadmap/M11-interactive-ux.md`: record `SettingsList` progress and the submenu deferral.

## Task 1: Tests First

- [ ] **Step 1: Write failing tests**

Create `crates/pi-tui/tests/settings_list.rs` with tests for:

```rust
use std::cell::RefCell;
use std::rc::Rc;

use pi_tui::{
    Component, KeybindingsManager, SettingItem, SettingsList, SettingsListOptions, StdinBuffer,
    TUI_KEYBINDINGS, visible_width,
};

fn keybindings() -> KeybindingsManager {
    KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default())
}

fn feed(list: &mut SettingsList, data: &str) {
    let mut buffer = StdinBuffer::new();
    let mut events = buffer.process(data);
    events.extend(buffer.flush());
    for event in events {
        list.handle_input(&event);
    }
}

#[test]
fn settings_list_renders_values_and_description_with_bounded_width() {
    let mut list = SettingsList::new(
        vec![
            SettingItem::new("model", "Model", "sonnet").description("Choose the active model"),
            SettingItem::new("theme", "Theme", "dark"),
        ],
        5,
        keybindings(),
    );

    let lines = list.render(24);

    assert_eq!(lines[0], "> Model  sonnet        ");
    assert!(lines.iter().any(|line| line.contains("Choose the active")));
    assert!(lines.iter().all(|line| visible_width(line) <= 24));
}

#[test]
fn settings_list_navigation_wraps_and_selected_item_updates() {
    let mut list = SettingsList::new(
        vec![
            SettingItem::new("model", "Model", "sonnet"),
            SettingItem::new("theme", "Theme", "dark"),
        ],
        5,
        keybindings(),
    );

    feed(&mut list, "\x1b[A");
    assert_eq!(list.selected_item().unwrap().id, "theme");
    feed(&mut list, "\x1b[B");
    assert_eq!(list.selected_item().unwrap().id, "model");
}

#[test]
fn settings_list_cycles_values_and_invokes_on_change() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let changes_for_callback = Rc::clone(&changes);
    let mut list = SettingsList::new(
        vec![SettingItem::new("theme", "Theme", "dark").values(["dark", "light"])],
        5,
        keybindings(),
    );
    list.set_on_change(Box::new(move |id, value| {
        changes_for_callback
            .borrow_mut()
            .push((id.to_string(), value.to_string()));
    }));

    feed(&mut list, "\r");

    assert_eq!(list.selected_item().unwrap().current_value, "light");
    assert_eq!(
        changes.borrow().as_slice(),
        &[("theme".to_string(), "light".to_string())]
    );
}

#[test]
fn settings_list_search_filters_with_fuzzy_matching() {
    let mut list = SettingsList::with_options(
        vec![
            SettingItem::new("model", "Model Selector", "sonnet"),
            SettingItem::new("theme", "Theme", "dark"),
        ],
        5,
        keybindings(),
        SettingsListOptions {
            enable_search: true,
        },
    );

    feed(&mut list, "mdl");

    assert_eq!(list.selected_item().unwrap().id, "model");
    let lines = list.render(24);
    assert!(lines.iter().any(|line| line.contains("Search: mdl")));
    assert!(lines.iter().all(|line| visible_width(line) <= 24));
}

#[test]
fn settings_list_escape_invokes_cancel_once() {
    let count = Rc::new(RefCell::new(0));
    let count_for_callback = Rc::clone(&count);
    let mut list = SettingsList::new(
        vec![SettingItem::new("theme", "Theme", "dark")],
        5,
        keybindings(),
    );
    list.set_on_cancel(Box::new(move || {
        *count_for_callback.borrow_mut() += 1;
    }));

    feed(&mut list, "\x1b");
    feed(&mut list, "\x1b");

    assert_eq!(*count.borrow(), 2);
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test settings_list
```

Expected: compile failure because the settings list types are not defined/exported yet.

## Task 2: Implementation

- [ ] **Step 1: Implement settings list types**

Create `crates/pi-tui/src/components/settings_list.rs` with:

- `SettingItem { id, label, description, current_value, values }`.
- Builder methods `new`, `description`, `values`.
- `SettingsListOptions { enable_search }` with `Default`.
- `SettingsList::new`, `with_options`, `selected_item`, `update_value`, `set_on_change`, `set_on_cancel`.
- Fuzzy filtering by `id`, `label`, `description`, and `current_value`.
- Width-bounded `render` with optional `Search: {query}` line, selected marker, aligned labels, current value, selected description, empty/no-match states, and hint line.
- Key handling for up/down/page up/page down, Enter and Space cycling, Escape/Ctrl+C cancel, typed search and backspace when search is enabled.

- [ ] **Step 2: Export public symbols**

Update `crates/pi-tui/src/components/mod.rs`:

```rust
mod settings_list;
pub use settings_list::{SettingItem, SettingsList, SettingsListOptions};
```

Update `crates/pi-tui/src/lib.rs` component re-export list to include `SettingItem`, `SettingsList`, and `SettingsListOptions`.

- [ ] **Step 3: Add public API smoke import**

Update `crates/pi-tui/tests/public_api.rs` to import and instantiate:

```rust
let _ = SettingsList::new(
    vec![SettingItem::new("theme", "Theme", "dark")],
    5,
    KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default()),
);
let _ = SettingsListOptions::default();
```

- [ ] **Step 4: Update roadmap**

Under M11 item 3, add:

```markdown
> 进度：`pi-tui` 已新增 `SettingsList`，支持设置项渲染、描述、键盘导航、fuzzy 搜索、值循环、change/cancel 回调；TS submenu 工厂待 selector/dialog 栈补齐后接入。
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
git add crates/pi-tui/src/components/settings_list.rs crates/pi-tui/src/components/mod.rs crates/pi-tui/src/lib.rs crates/pi-tui/tests/settings_list.rs crates/pi-tui/tests/public_api.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-settings-list.md
git commit -m "feat: add tui settings list"
```

Expected: commit succeeds with only this component-library slice included.
