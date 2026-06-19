# M11 Fuzzy SelectList Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the TypeScript TUI fuzzy matching behavior into `pi-tui` and use it for `SelectList` filtering/sorting.

**Architecture:** Add a small `fuzzy` module to `pi-tui` with `fuzzy_match` and `fuzzy_filter_indices`, mirroring `pi/packages/tui/src/fuzzy.ts`. Keep `SelectList` responsible for building searchable text from item value, label, and description, but delegate matching and score sorting to the shared module.

**Tech Stack:** Rust 2024, `pi-tui` component tests, TypeScript reference `pi/packages/tui/src/fuzzy.ts`.

---

## File Structure

- Create `crates/pi-tui/src/fuzzy.rs`: fuzzy scoring and index filtering helpers.
- Modify `crates/pi-tui/src/lib.rs`: export `FuzzyMatch`, `fuzzy_match`, and `fuzzy_filter_indices`.
- Modify `crates/pi-tui/src/components/select_list.rs`: replace substring matching with fuzzy scoring over item value/label/description text.
- Modify `crates/pi-tui/tests/select_list.rs`: add behavior tests for non-contiguous matching and score ordering.
- Create `crates/pi-tui/tests/fuzzy.rs`: table-driven tests for matching, sorting, token behavior, and alpha/numeric swapped queries.
- Modify `docs/roadmap/M11-interactive-ux.md`: record fuzzy matching progress after verification passes.

## Task 1: Fuzzy Module

- [ ] **Step 1: Write failing fuzzy tests**

Create `crates/pi-tui/tests/fuzzy.rs` with tests for:

```rust
use pi_tui::{fuzzy_filter_indices, fuzzy_match};

#[test]
fn fuzzy_match_allows_ordered_non_contiguous_characters() {
    let matched = fuzzy_match("mdl", "model-selector");
    assert!(matched.matches);
    assert!(!fuzzy_match("mld", "model-selector").matches);
}

#[test]
fn fuzzy_match_prefers_exact_and_consecutive_matches() {
    let exact = fuzzy_match("model", "model");
    let spaced = fuzzy_match("model", "m-o-d-e-l");
    assert!(exact.matches);
    assert!(spaced.matches);
    assert!(exact.score < spaced.score);
}

#[test]
fn fuzzy_filter_indices_requires_all_tokens_and_sorts_by_score() {
    let items = vec!["model selector", "session model", "settings"];
    let indices = fuzzy_filter_indices(&items, "mod sel", |item| *item);
    assert_eq!(indices, vec![0]);
}

#[test]
fn fuzzy_match_supports_swapped_letter_digit_queries() {
    assert!(fuzzy_match("gpt5", "gpt-5").matches);
    assert!(fuzzy_match("5gpt", "gpt-5").matches);
}
```

- [ ] **Step 2: Run fuzzy tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test fuzzy
```

Expected: compile failure because the module and exports do not exist.

- [ ] **Step 3: Implement `crates/pi-tui/src/fuzzy.rs` and exports**

Implement the TS scoring rules:

- Empty query matches with score `0.0`.
- Characters must match in order.
- Consecutive matches reduce score by `consecutive_matches * 5.0`.
- Gaps add `(gap_len * 2.0)`.
- Word boundary matches at string start or after whitespace, `-`, `_`, `.`, `/`, `:` reduce score by `10.0`.
- Later character positions add `i * 0.1`.
- Exact lowercase text match reduces score by `100.0`.
- Letter+digit or digit+letter query may match with groups swapped, with `+5.0` score penalty.
- Space-separated query tokens must all match; return original indices sorted by total score.

- [ ] **Step 4: Run fuzzy tests and verify they pass**

Run:

```bash
cargo test -p pi-tui --test fuzzy
```

Expected: all fuzzy tests pass.

## Task 2: SelectList Integration

- [ ] **Step 1: Write failing SelectList tests**

Add to `crates/pi-tui/tests/select_list.rs`:

```rust
#[test]
fn select_list_uses_fuzzy_filtering_for_non_contiguous_input() {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let mut list = SelectList::new(
        vec![
            SelectItem::new("model-selector", "Model Selector"),
            SelectItem::new("session", "Session"),
        ],
        5,
        keybindings,
    );

    list.set_filter("mdl");
    assert_eq!(list.selected_item().unwrap().value, "model-selector");
}

#[test]
fn select_list_orders_fuzzy_matches_by_score() {
    let keybindings = KeybindingsManager::new(TUI_KEYBINDINGS.clone(), Default::default());
    let mut list = SelectList::new(
        vec![
            SelectItem::new("my-model", "My Model"),
            SelectItem::new("model", "Model"),
        ],
        5,
        keybindings,
    );

    list.set_filter("model");
    assert_eq!(list.selected_item().unwrap().value, "model");
}
```

- [ ] **Step 2: Run SelectList tests and verify they fail**

Run:

```bash
cargo test -p pi-tui --test select_list
```

Expected: the non-contiguous match or ordering test fails under substring filtering.

- [ ] **Step 3: Integrate fuzzy sorting in SelectList**

In `select_list.rs`, remove `item_matches` and use `fuzzy_filter_indices(&self.items, &self.filter, searchable_text)` in `rebuild_filter()`.

Use:

```rust
fn searchable_text(item: &SelectItem) -> String {
    match &item.description {
        Some(description) => format!("{} {} {}", item.value, item.label, description),
        None => format!("{} {}", item.value, item.label),
    }
}
```

- [ ] **Step 4: Run SelectList tests and verify they pass**

Run:

```bash
cargo test -p pi-tui --test select_list
```

Expected: all SelectList tests pass.

## Task 3: Roadmap, Verification, Commit

- [ ] **Step 1: Update `docs/roadmap/M11-interactive-ux.md`**

Under “搜索与补全”, add:

```markdown
> 进度：`pi-tui` 已有 TS-parity fuzzy scoring/filtering，`SelectList` 已切到 fuzzy 匹配和评分排序；autocomplete 仍待移植。
```

- [ ] **Step 2: Verify**

Run:

```bash
cargo fmt --check
cargo test -p pi-tui
env -u NO_COLOR TERM=xterm-256color cargo test --workspace
cargo check --workspace
```

Expected: all pass. The workspace test keeps `NO_COLOR` unset because this shell exports `NO_COLOR=1`, while existing markdown tests assert color-enabled ANSI output.

- [ ] **Step 3: Commit**

Run:

```bash
git add crates/pi-tui/src/fuzzy.rs crates/pi-tui/src/lib.rs crates/pi-tui/src/components/select_list.rs crates/pi-tui/tests/fuzzy.rs crates/pi-tui/tests/select_list.rs docs/roadmap/M11-interactive-ux.md docs/superpowers/plans/2026-06-20-m11-fuzzy-select-list.md
git commit -m "feat: add fuzzy select list filtering"
```

Expected: commit succeeds with only fuzzy-select-list files included.
