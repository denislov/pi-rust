# Settings Menu — Tier 2 实施计划

> **目标：** 在 Rust interactive TUI 的设置菜单中新增 9 个设置项，扩展配置数据模型，实现与 TS 的配置字段对齐。

## 概览

当前 Rust 设置菜单已有 7 项（Theme、Auto compact、Transport、Steering mode、Follow-up mode、Show images、Terminal progress）。

Tier 2 新增 9 项，分为两组：

| 组 | 数量 | 说明 |
|----|------|------|
| **A — 新增标量字段** | 6 | 在 `PartialSettings` / `Settings` 中直接添加 flat 字段 |
| **B — 扩展 TerminalSettings** | 3 | 扩展 `PartialTerminal` / `TerminalSettings` 结构体 |

---

## A 组：标量字段（PartialSettings / Settings）

这 6 项直接作为 `PartialSettings` 的 `Option` 字段和 `Settings` 的 resolved 字段。

### A1: `hide_thinking_block`

| 属性 | 值 |
|------|-----|
| TS 来源 | `Settings.hideThinkingBlock` |
| Rust Partial | `hide_thinking_block: Option<bool>` |
| Rust Resolved | `hide_thinking_block: bool` |
| 默认值 | `false` |
| 设置项 | `"hide_thinking"` / "Hide thinking" / on, off |
| 描述 | "Hide thinking blocks in assistant responses" |

### A2: `collapse_changelog`

| 属性 | 值 |
|------|-----|
| TS 来源 | `Settings.collapseChangelog` |
| Rust Partial | `collapse_changelog: Option<bool>` |
| Rust Resolved | `collapse_changelog: bool` |
| 默认值 | `false` |
| 设置项 | `"collapse_changelog"` / "Collapse changelog" / on, off |
| 描述 | "Show condensed changelog after updates" |

### A3: `quiet_startup`

| 属性 | 值 |
|------|-----|
| TS 来源 | `Settings.quietStartup` |
| Rust Partial | `quiet_startup: Option<bool>` |
| Rust Resolved | `quiet_startup: bool` |
| 默认值 | `false` |
| 设置项 | `"quiet_startup"` / "Quiet startup" / on, off |
| 描述 | "Disable verbose printing at startup" |

### A4: `enable_skill_commands`

| 属性 | 值 |
|------|-----|
| TS 来源 | `Settings.enableSkillCommands` |
| Rust Partial | `enable_skill_commands: Option<bool>` |
| Rust Resolved | `enable_skill_commands: bool` |
| 默认值 | `true` |
| 设置项 | `"enable_skill_commands"` / "Skill commands" / on, off |
| 描述 | "Register skills as /skill:name commands" |

### A5: `double_escape_action`

| 属性 | 值 |
|------|-----|
| TS 来源 | `Settings.doubleEscapeAction` |
| Rust Partial | `double_escape_action: Option<String>` |
| Rust Resolved | `double_escape_action: String` |
| 默认值 | `"tree"` |
| 可选值 | `["tree", "fork", "none"]` |
| 设置项 | `"double_escape_action"` / "Double-escape action" |
| 描述 | "Action when pressing Escape twice with empty editor" |

### A6: `tree_filter_mode`

| 属性 | 值 |
|------|-----|
| TS 来源 | `Settings.treeFilterMode` |
| Rust Partial | `tree_filter_mode: Option<String>` |
| Rust Resolved | `tree_filter_mode: String` |
| 默认值 | `"default"` |
| 可选值 | `["default", "no-tools", "user-only", "labeled-only", "all"]` |
| 设置项 | `"tree_filter_mode"` / "Tree filter mode" |
| 描述 | "Default filter when opening /tree" |

---

## B 组：扩展 TerminalSettings

### B1: `clear_on_shrink`

| 属性 | 值 |
|------|-----|
| TS 来源 | `TerminalSettings.clearOnShrink` |
| Rust PartialTerminal | `clear_on_shrink: Option<bool>` |
| Rust TerminalSettings | `clear_on_shrink: bool` |
| 默认值 | `false` |
| 设置项 | `"clear_on_shrink"` / "Clear on shrink" / on, off |
| 描述 | "Clear empty rows when content shrinks (may cause flicker)" |

### B2: `auto_resize_images`

| 属性 | 值 |
|------|-----|
| TS 来源 | `ImageSettings.autoResize` |
| Rust PartialTerminal | `auto_resize_images: Option<bool>` |
| Rust TerminalSettings | `auto_resize_images: bool` |
| 默认值 | `true` |
| 设置项 | `"auto_resize_images"` / "Auto-resize images" / on, off |
| 描述 | "Resize large images to 2000×2000 max for better model compatibility" |

### B3: `block_images`

| 属性 | 值 |
|------|-----|
| TS 来源 | `ImageSettings.blockImages` |
| Rust PartialTerminal | `block_images: Option<bool>` |
| Rust TerminalSettings | `block_images: bool` |
| 默认值 | `false` |
| 设置项 | `"block_images"` / "Block images" / on, off |
| 描述 | "Prevent images from being sent to LLM providers" |

---

## 改动文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `pi-coding-agent/src/config/settings.rs` | 修改 | `PartialSettings` 新增 6 个字段，`PartialTerminal` 新增 3 个字段，`TerminalSettings` 新增 3 个字段，`Settings` 新增 6+3=9 个字段，更新 merge/resolve，更新现有测试 |
| `pi-coding-agent/src/interactive/root.rs` | 修改 | `build_settings_list` 添加 9 个 SettingItem，`apply_settings_value` 添加 9 个 match arm |
| `pi-coding-agent/src/interactive/app.rs` | 修改 | 新增 9 个设置项测试 |

---

## 实施步骤

### Step 1: 扩展数据模型

在 `settings.rs` 中：

**PartialTerminal** 新增字段：
```rust
pub struct PartialTerminal {
    pub show_images: Option<bool>,
    pub show_progress: Option<bool>,
    pub clear_on_shrink: Option<bool>,
    pub auto_resize_images: Option<bool>,
    pub block_images: Option<bool>,
}
```

**TerminalSettings** 新增字段：
```rust
pub struct TerminalSettings {
    pub show_images: bool,
    pub show_progress: bool,
    pub clear_on_shrink: bool,
    pub auto_resize_images: bool,
    pub block_images: bool,
}
```

**PartialSettings** 新增字段：
```rust
pub struct PartialSettings {
    // ... existing fields ...
    pub hide_thinking_block: Option<bool>,
    pub collapse_changelog: Option<bool>,
    pub quiet_startup: Option<bool>,
    pub enable_skill_commands: Option<bool>,
    pub double_escape_action: Option<String>,
    pub tree_filter_mode: Option<String>,
}
```

**Settings** 新增字段：
```rust
pub struct Settings {
    // ... existing fields ...
    pub hide_thinking_block: bool,
    pub collapse_changelog: bool,
    pub quiet_startup: bool,
    pub enable_skill_commands: bool,
    pub double_escape_action: String,
    pub tree_filter_mode: String,
}
```

**`merge_terminal`** 更新：
```rust
fn merge_terminal(...) -> Option<PartialTerminal> {
    Some(PartialTerminal {
        show_images: o.show_images.or(b.show_images),
        show_progress: o.show_progress.or(b.show_progress),
        clear_on_shrink: o.clear_on_shrink.or(b.clear_on_shrink),
        auto_resize_images: o.auto_resize_images.or(b.auto_resize_images),
        block_images: o.block_images.or(b.block_images),
    })
}
```

**`PartialSettings::merge`** 更新——新增字段的合并：
```rust
hide_thinking_block: over.hide_thinking_block.or(self.hide_thinking_block),
collapse_changelog: over.collapse_changelog.or(self.collapse_changelog),
quiet_startup: over.quiet_startup.or(self.quiet_startup),
enable_skill_commands: over.enable_skill_commands.or(self.enable_skill_commands),
double_escape_action: over.double_escape_action.or(self.double_escape_action),
tree_filter_mode: over.tree_filter_mode.or(self.tree_filter_mode),
```

**`resolve`** 更新：
```rust
TerminalSettings {
    show_images: t.show_images.unwrap_or(true),
    show_progress: t.show_progress.unwrap_or(true),
    clear_on_shrink: t.clear_on_shrink.unwrap_or(false),
    auto_resize_images: t.auto_resize_images.unwrap_or(true),
    block_images: t.block_images.unwrap_or(false),
},
// ... plus:
hide_thinking_block: self.hide_thinking_block.unwrap_or(false),
collapse_changelog: self.collapse_changelog.unwrap_or(false),
quiet_startup: self.quiet_startup.unwrap_or(false),
enable_skill_commands: self.enable_skill_commands.unwrap_or(true),
double_escape_action: self.double_escape_action.unwrap_or_else(|| "tree".to_string()),
tree_filter_mode: self.tree_filter_mode.unwrap_or_else(|| "default".to_string()),
```

**测试更新**：更新 `defaults_applied_on_empty` 等测试来验证新默认值。

### Step 2: 添加 UI 菜单项

在 `root.rs` 的 `build_settings_list` 中，在现有 7 项之后添加 9 个新的 `SettingItem`。设置列表高度从 8 增加到 12。

在 `apply_settings_value` 中添加对应的 `match` arm。

### Step 3: 添加测试

在 `app.rs` 中，每个设置项添加一个测试，遵循 `settings_menu_*_cycles_and_reports_update` 模式（导航到对应项、按 Enter 切换值、验证变更和 `settings_update` 事件）。

### Step 4: 运行验证

```bash
cargo test -p pi-coding-agent --lib interactive::app
cargo test -p pi-coding-agent --lib config::settings::tests
cargo check --workspace
```

---

## 菜单顺序

按 TS `settings-selector.ts` 的排列顺序组织，最终菜单：

```
  Theme               dark / light
  Auto compact        on / off
  Transport           sse / websocket / websocket-cached / auto
  Steering mode       one-at-a-time / all
  Follow-up mode      one-at-a-time / all
  Show images         on / off
  Auto-resize images  on / off                   ← 新增 (B2)
  Block images        on / off                   ← 新增 (B3)
  Skill commands      on / off                   ← 新增 (A4)
  Hide thinking       on / off                   ← 新增 (A1)
  Collapse changelog  on / off                   ← 新增 (A2)
  Quiet startup       on / off                   ← 新增 (A3)
  Clear on shrink     on / off                   ← 新增 (B1)
  Double-escape action tree / fork / none        ← 新增 (A5)
  Tree filter mode    default / no-tools / ...   ← 新增 (A6)
  Terminal progress   on / off
```
