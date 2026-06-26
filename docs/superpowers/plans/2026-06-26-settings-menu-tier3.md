# Settings Menu — Tier 3 实施方案

> **目标：** 补齐 Rust settings 数据模型中缺失的 TS 设置项，规划子菜单/复杂 UI 的接入路径，评估运行时接线的优先级与成本。

---

## 概览

Tier 1（7 项）和 Tier 2（9 项）覆盖了 `PartialSettings` / `Settings` 中的核心字段。Tier 3 处理三类残局：

| 类别 | 数量 | 说明 |
|------|------|------|
| **A — 缺失的数据模型字段** | 15 | TS 有、Rust 完全没有的配置字段 |
| **B — 子菜单 / 复杂 UI** | 3 | 需要 submenu 或嵌套结构才能暴露的设置 |
| **C — 运行时接线** | 8 | 数据模型 + UI 已有，但 runtime 未消费的设置 |

---

## A — 缺失的数据模型字段

以下字段存在于 TS `Settings` 接口中，但 Rust `PartialSettings` / `Settings` 中完全没有。按助记分组：

### A1: Shell 和命令相关

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `shell_path` | `string` | 系统默认 shell | 自定义 shell 路径（如 Windows Cygwin 用户） |
| `shell_command_prefix` | `string` | `""` | 每条 bash 命令前追加的前缀（如 `shopt -s expand_aliases`） |
| `npm_command` | `string[]` | `["npm"]` | npm 包查找/安装的命令，argv 格式 |

**实现成本**：低。三个 `Option<String>` / `Option<Vec<String>>` 字段，标准 merge/resolve 模式。

**优先级**：低。shell 相关功能在 Rust 中尚为 placeholder。

### A2: 图片相关

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `terminal.image_width_cells` | `number` | `60` | 行内图片宽度（终端单元数） |

**实现成本**：低。`PartialTerminal` 新增 `image_width_cells: Option<u32>`。

**注意事项**：TS 中此设置仅在终端支持图片时显示（`getCapabilities().images`）。Rust 端也应检测终端能力。

### A3: 编辑器 / TUI 相关

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `editor_padding_x` | `number` | `0` | 输入编辑器的水平内边距（0-3） |
| `autocomplete_max_visible` | `number` | `5` | 自动补全下拉菜单的最大可见项数（3-20） |
| `show_hardware_cursor` | `boolean` | `false` | 显示终端硬件光标（IME 支持） |

**实现成本**：中。这些字段的**存储**简单，但**消费端**需要 `pi-tui` 的 `Editor` / 自动补全组件支持对应配置。如果组件尚未实现参数化，接线成本高于字段定义。

### A4: 项目信任 / 扩展

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `default_project_trust` | `"ask"\|"always"\|"never"` | `"ask"` | 无扩展或已保存信任决策时的回退行为 |
| `extensions` | `string[]` | `[]` | 本地扩展文件路径或目录 |

**实现成本**：中。`default_project_trust` 数据模型简单（`Option<String>`），但消费端涉及扩展系统启动决策。`extensions` 已是 `Vec<String>` 模式，与 `skills`/`prompts`/`themes` 相同。

### A5: 遥测 / 分析

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `enable_install_telemetry` | `boolean` | `true` | 更新后发送匿名版本/更新 ping |
| `enable_analytics` | `boolean` | `false` | 选择性分析数据共享 |
| `tracking_id` | `string` | 自动生成 | 分析追踪标识符 |

**实现成本**：这些字段**不推荐立即添加**。Rust 中还没有遥测基础设施，添加存储而无消费端会增加维护负担。

### A6: HTTP / 网络

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `http_proxy` | `string` | `""` | 代理 URL，设为 `HTTP_PROXY` / `HTTPS_PROXY` |
| `http_idle_timeout_ms` | `number` | 取决于 provider | HTTP 空闲超时（毫秒）；0=禁用 |
| `websocket_connect_timeout_ms` | `number` | 取决于 provider | WebSocket 连接超时（毫秒）；0=禁用 |

**实现成本**：中。数据模型简单，但消费端需要 `reqwest` client 配置支持。`pi-agent-core` 已经使用 `reqwest`，但超时设置目前是硬编码的。

### A7: 杂项

| 字段 | TS 类型 | 默认值 | 说明 |
|------|---------|--------|------|
| `branch_summary` | `BranchSummarySettings` | `{ reserveTokens: 16384, skipPrompt: false }` | 分支摘要设置 |
| `enabled_models` | `string[]` | `[]` | 模型轮换的 model patterns |
| `thinking_budgets` | `ThinkingBudgetsSettings` | `{ ... }` | 各 thinking level 的自定义 token 预算 |
| `markdown.code_block_indent` | `string` | `"  "` | 代码块缩进 |
| `warnings` | `WarningSettings` | `{ anthropicExtraUsage: true }` | 单个警告的启用/禁用 |
| `last_changelog_version` | `string` | `undefined` | 追踪字段，非用户可配 |

**优先级**：
- `enabled_models` — 高，对应 model rotation 功能
- `thinking_budgets` — 中，think block 功能增强
- `warnings` — 中，影响用户体验
- `branch_summary` — 低，分支摘要尚未移植
- `markdown.code_block_indent` — 低，纯展示

---

## B — 子菜单 / 复杂 UI

当前 Rust `SettingsList` 的 `SettingItem` 只支持简单的值循环（`values`）。TS 中三类设置使用了子菜单：

### B1: Thinking Level 子菜单

**TS 实现**：`SelectSubmenu` 组件，展示六个级别及其描述。

**Rust 现状**：Thinking level 已有设置菜单暴露方式？

实际上，thinking level 目前**不是** settings menu 中的一项，而是通过 `/model model_id:thinking_level` 设置的。TS 的 settings 菜单中有独立的 thinking level 子菜单，可以让用户在当前模型支持的所有 thinking level 间切换。

**方案**：在 settings 菜单中新增一个触发 `/model` 命令的项，或让 `SettingItem` 支持 `submenu` 回调。后者需要扩展 `SettingItem` 类型。

### B2: Theme 子菜单（自动模式）

**TS 实现**：`ThemeSubmenu` 组件，支持单主题模式 + 自动模式（light/dark 分别设置）。

**Rust 现状**：`build_settings_list` 中 `theme` 项只允许 `["dark", "light"]`，没有自动模式。

**方案**：
- 短期：保持简单，只做 dark/light
- 中期：扩展 theme 值为 `["dark", "light", "auto"]`，`auto` 时读取终端色彩模式
- 长期：实现完整的 light/dark 分别设置 + terminal theme detection

### B3: Warning 子菜单

**TS 实现**：`WarningSettingsSubmenu` 组件，内嵌在 settings 列表中。

**方案**：暂缓。Warning 功能的意义依赖于其他功能的完整性（如 subscription auth）。

---

## C — 运行时接线（路线图）

| 设置项 | 接线位置 | 优先级 | 估算成本 |
|--------|---------|--------|---------|
| `steering_mode` | `SessionPromptOptions` → `AgentConfig` | 🔴 高 | 小（约 10 行） |
| `follow_up_mode` | `SessionPromptOptions` → `AgentConfig` | 🔴 高 | 小（约 10 行） |
| `auto_compaction` | 传递给 compaction 逻辑 | 🔴 高 | 中（需理解 compaction 路径） |
| `transport` | `prompt_context` → provider 创建 | 🟡 中 | 中 |
| `show_images` | TUI markdown 渲染时检查 | 🟡 中 | 小 |
| `auto_resize_images` | 图片处理 pipeline | 🟢 低 | 中（需图片处理基础设施） |
| `block_images` | 发送图片前 gate | 🟢 低 | 小（一行检查） |
| `hide_thinking_block` | markdown 渲染时过滤 | 🟢 低 | 小（渲染时检查 flag） |
| `show_progress` | 需要 OSC 9;4 支持 | 🟢 低 | 大（需 TUI 新功能） |

---

## 推荐的实施批次

### 批次 1（近期，低风险）

**A 组中纯数据模型的字段**（`shell_path`, `shell_command_prefix`, `npm_command`, `image_width_cells`, `http_proxy`, `http_idle_timeout_ms`, `websocket_connect_timeout_ms`）：
- 仅扩展 `PartialSettings` / `Settings` 结构体
- 不加入 settings 菜单（`build_settings_list`）
- 不接 runtime
- 让 TOML 配置文件可以写入这些值
- **成本**：约 1 小时

**C 组中高优先级接线**（`steering_mode`, `follow_up_mode`）：
- 在 `SessionPromptOptions` 中添加字段
- 从 `prompt_context.settings` 读取并传入 AgentConfig
- **成本**：约 30 分钟

### 批次 2（中期）

- `enabled_models` — 数据模型 + 与 model rotation 对接
- `warnings` — 数据模型 + settings menu 项
- `hide_thinking_block` — wiring
- `block_images` — wiring

### 批次 3（长期）

- Theme 自动模式（terminal theme detection）
- Thinking level 子菜单
- `editor_padding_x`, `autocomplete_max_visible`, `show_hardware_cursor` — 需要 pi-tui 组件支持
- 遥测 / 分析设置（配合遥测基础设施）

---

## 文件改动清单（批次 1）

| 文件 | 改动 | 规模 |
|------|------|------|
| `pi-coding-agent/src/config/settings.rs` | `PartialSettings`/`Settings` 新增字段 + merge/resolve | ~50 行 |
| `pi-coding-agent/src/protocol/session_runner.rs` | `SessionPromptOptions` + AgentConfig 传入 steering/follow_up | ~20 行 |
| `pi-coding-agent/src/interactive/loop.rs` | `start_prompt_task` 传递 steering/follow_up settings | ~10 行 |
| `pi-coding-agent/src/config/settings.rs` | 现有测试更新 + 新增字段测试 | ~30 行 |
