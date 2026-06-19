# M11 — 交互体验补全

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)、[M9](M9-agent-harness.md)、[M10](M10-resources-input.md)
> 定位：**交互模式从"能跑"到"好用"**。含交互发布门 TUI-7。

## 目标
补齐 slash 命令、交互组件、`pi-tui` 组件库与主题系统，让交互模式达到 pi 的体验水准。

## 待实现项

### 1. slash 命令（2 → 21）
当前仅 `/quit`、`/help`。补 19 个（TS：`coding-agent/src/core/slash-commands.ts`）：
`/model` `/scoped-models` `/export` `/import` `/share` `/copy` `/name` `/session` `/changelog`
`/hotkeys` `/fork` `/clone` `/tree` `/login` `/logout` `/new` `/compact` `/resume` `/reload`
> 其中 `/export`/`/import`/`/share`/`/copy` 的后端实现归 [M13](M13-peripherals.md)；本里程碑做命令框架 + 其余命令。
> 注：`docs/superpowers/specs/2026-06-19-pi-coding-agent-slash-commands-design.md` 已有设计稿，可直接接续。

### 2. 交互组件（~40 个选择器/对话框）
- model-selector、oauth-selector、settings-selector、login-dialog、session-selector、tree-selector、theme-selector、config-selector、thinking-selector、scoped-models-selector 等。
- TS：`coding-agent/src/modes/interactive/components/`（39 个文件）。Rust 当前 0 个自定义组件。

### 3. `pi-tui` 组件库补全
- **`Loader` / `CancellableLoader`**：把当前硬编码在 coding-agent 的 spinner 抽成 pi-tui 可复用组件（CancellableLoader 支持 Esc + `AbortSignal`）。TS：`tui/src/components/loader.ts`、`cancellable-loader.ts`。
- `Box`（padding + 背景）、`TruncatedText`、`SettingsList`（可搜索/值循环/子菜单）。
- `Image`（依赖下方终端图像协议）。

### 4. 搜索与补全
- **fuzzy 匹配**（评分排序，替换 SelectList 现有的 `starts_with` 前缀匹配）。TS：`tui/src/fuzzy.ts`。
- **autocomplete**（路径/env/命令建议）。TS：`tui/src/autocomplete.ts`。

### 5. 主题系统（TUI-8 剩余）
- 256 色 / RGB（当前仅 8 色 ANSI）。
- dark / light / custom 主题；各组件接受 theme（MarkdownTheme/SelectListTheme/EditorTheme…）。
- 能力探测（`COLORTERM=truecolor/24bit`、`TERM`）。TS：`tui/src/terminal-image.ts` 的 `detectCapabilities`。

### 6. 高级文本处理
- `wrapTextWithAnsi`（ANSI 保留的换行 + grapheme 分割 + 省略号/padding）。
- emoji 探测 + 宽度缓存（LRU）。word-navigation 增强（对标 `Intl.Segmenter`，当前为简化字符级）。

### 7. 终端图像协议
- Kitty / iTerm2 编码、PNG/JPEG/GIF/WebP 尺寸解析、cell 尺寸、`renderImage`/`deleteKittyImage`。TS：`tui/src/terminal-image.ts`。

### 8. TUI-7 跨终端 smoke 套件（**交互发布门**）
- tmux 脚本 + 终端行为记录表，覆盖 wezterm/kitty/iTerm2/Terminal.app/GNOME Terminal/tmux/SSH。
- 作为交互模式"可发布"的门槛。

## 验收 / 测试（离线优先）
- 组件：用 `VirtualTerminal` 后端断言渲染输出与按键行为。
- fuzzy/autocomplete：表驱动单测。
- 主题/能力探测：注入 env 断言降级路径（NO_COLOR/dumb/truecolor）。
- TUI-7：脚本化记录，人工核对表。
