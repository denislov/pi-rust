# M11 — 交互体验补全

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)、[M9](M9-agent-harness.md)、[M10](M10-resources-input.md)
> 定位：**交互模式从"能跑"到"好用"**。含交互发布门 TUI-7。

## 目标
补齐 slash 命令、交互组件、`pi-tui` 组件库与主题系统，让交互模式达到 pi 的体验水准。

## 实际推进状态

M11 已按可离线验证的交互基础设施路径推进完成。当前落地范围：

- slash command registry 已覆盖 TS built-in 命令名，`/help`、`/quit`、`/name`、`/session`、`/hotkeys`、`/model` 具备本地行为；依赖外设/会话后端的命令返回显式未实现提示，留给对应里程碑接管后端行为。
- `pi-tui` 已具备可复用 `Loader` / `CancellableLoader`、`Box`、`TruncatedText`、`SettingsList`、`Image`、`SelectorDialog` 等组件基础。
- fuzzy 搜索和同步 autocomplete 已移植，覆盖 slash command、路径/附件、环境变量建议。
- 主题系统已落地 dark/light/custom palette，并为 Markdown、SelectList、SettingsList、Editor 提供 theme 结构；Markdown 与 SelectList 已接入组件 theme 参数。
- 高级文本处理新增 ANSI-aware wrapping 与 ellipsis truncation，保留 SGR 序列并按 grapheme / wide width 控制行宽。
- 终端图像协议新增 Kitty / iTerm2 编码、删除序列、能力探测、PNG/JPEG/GIF/WebP 尺寸解析、cell size 计算和 `Image` fallback。
- TUI-7 smoke suite 已落地 tmux 脚本和人工记录文档，可捕获启动、首字符、清文本、宽字符、resize、`/help`、退出与可选真实 provider stream。

剩余交互产品化项主要是把所有选择器逐一接入 coding-agent 的具体工作流，以及 M13 外设类命令的真实后端行为；M11 侧的可复用 TUI/交互基础设施已就位。

## 待实现项

### 1. slash 命令（2 → 21）
当前仅 `/quit`、`/help`。补 19 个（TS：`coding-agent/src/core/slash-commands.ts`）：
`/model` `/scoped-models` `/export` `/import` `/share` `/copy` `/name` `/session` `/changelog`
`/hotkeys` `/fork` `/clone` `/tree` `/login` `/logout` `/new` `/compact` `/resume` `/reload`
> 其中 `/export`/`/import`/`/share`/`/copy` 的后端实现归 [M13](M13-peripherals.md)；本里程碑做命令框架 + 其余命令。
> 注：`docs/superpowers/specs/2026-06-19-pi-coding-agent-slash-commands-design.md` 已有设计稿，可直接接续。
> 进度：Rust 已有内置 slash command registry，覆盖 TS 的 21 个 built-in 命令名（含 `/settings`、`/quit`）；`/help`、`/quit`、`/name`、`/session`、`/hotkeys`、`/model` 已有本地行为，其余会话/外设命令先返回显式未实现提示，后续随对应后端里程碑补齐。

### 2. 交互组件（~40 个选择器/对话框）
- model-selector、oauth-selector、settings-selector、login-dialog、session-selector、tree-selector、theme-selector、config-selector、thinking-selector、scoped-models-selector 等。
- TS：`coding-agent/src/modes/interactive/components/`（39 个文件）。Rust 当前 0 个自定义组件。
> 进度：Rust 已新增通用 `SelectorDialog`，并已有 `/model` selector 状态；selector/dialog 基础 API 覆盖标题、help、fuzzy SelectList、confirm/cancel callback。具体 theme/session/tree/login 等业务对话框后续可在此基础上接入。

### 3. `pi-tui` 组件库补全
- **`Loader` / `CancellableLoader`**：把当前硬编码在 coding-agent 的 spinner 抽成 pi-tui 可复用组件（CancellableLoader 支持 Esc + `AbortSignal`）。TS：`tui/src/components/loader.ts`、`cancellable-loader.ts`。
- `Box`（padding + 背景）、`TruncatedText`、`SettingsList`（可搜索/值循环/子菜单）。
- `Image`（依赖下方终端图像协议）。
> 进度：`pi-tui` 已新增可复用 `Loader` / `CancellableLoader`，支持 deterministic frame tick、消息更新、自定义/隐藏 indicator、宽度裁剪和 Escape/Ctrl+C 取消回调；coding-agent 侧硬编码 spinner 迁移仍待接入。
> 进度：`pi-coding-agent` interactive footer spinner 已迁移为复用 `pi_tui::Loader`，不再在 coding-agent 侧维护独立 spinner frame 表。
> 进度：`pi-tui` 已新增 `Box` / `TruncatedText` 基础组件，覆盖 padding、单行截断、宽度约束和背景回调；后续 selectors/dialogs 可直接复用。
> 进度：`pi-tui` 已新增 `SettingsList`，支持设置项渲染、描述、键盘导航、fuzzy 搜索、值循环、change/cancel 回调；TS submenu 工厂待 selector/dialog 栈补齐后接入。
> 进度：`pi-tui` 已新增 `Image` 组件，在 Kitty/iTerm2 可用时渲染协议序列，否则输出宽度受限 fallback 文本。

### 4. 搜索与补全
- **fuzzy 匹配**（评分排序，替换 SelectList 现有的 `starts_with` 前缀匹配）。TS：`tui/src/fuzzy.ts`。
- **autocomplete**（路径/env/命令建议）。TS：`tui/src/autocomplete.ts`。
> 进度：`pi-tui` 已有 TS-parity fuzzy scoring/filtering，`SelectList` 已切到 fuzzy 匹配和评分排序；autocomplete 仍待移植。
> 进度：`pi-tui` 已新增同步 `CombinedAutocompleteProvider`，支持 slash command、路径/`@` 附件、`$ENV` 建议和 completion application；fd 递归 fuzzy 文件搜索后续优化。

### 5. 主题系统（TUI-8 剩余）
- 256 色 / RGB（当前仅 8 色 ANSI）。
- dark / light / custom 主题；各组件接受 theme（MarkdownTheme/SelectListTheme/EditorTheme…）。
- 能力探测（`COLORTERM=truecolor/24bit`、`TERM`）。TS：`tui/src/terminal-image.ts` 的 `detectCapabilities`。
> 进度：`pi-tui` style 已支持 ANSI 256/RGB SGR 输出与可注入的颜色能力探测（NO_COLOR、dumb、truecolor/24bit、*-256color）；dark/light/custom theme 对象已落地，Markdown/SelectList 已接收 theme 参数，SettingsList/Editor theme 结构已为后续接入预留。

### 6. 高级文本处理
- `wrapTextWithAnsi`（ANSI 保留的换行 + grapheme 分割 + 省略号/padding）。
- emoji 探测 + 宽度缓存（LRU）。word-navigation 增强（对标 `Intl.Segmenter`，当前为简化字符级）。
> 进度：已新增 `wrap_text_with_ansi` 与 `truncate_to_width_with_ellipsis`，覆盖 ANSI SGR 保留、literal newline、grapheme、wide emoji 和省略号宽度。emoji 探测缓存和 word-navigation 细化仍可作为后续 polish。

### 7. 终端图像协议
- Kitty / iTerm2 编码、PNG/JPEG/GIF/WebP 尺寸解析、cell 尺寸、`renderImage`/`deleteKittyImage`。TS：`tui/src/terminal-image.ts`。
> 进度：已新增 `terminal_image` 模块，覆盖 Kitty/iTerm2 编码、Kitty 删除序列、环境能力探测、PNG/JPEG/GIF/WebP 尺寸解析、cell size 计算与 `render_image`。

### 8. TUI-7 跨终端 smoke 套件（**交互发布门**）
- tmux 脚本 + 终端行为记录表，覆盖 wezterm/kitty/iTerm2/Terminal.app/GNOME Terminal/tmux/SSH。
- 作为交互模式"可发布"的门槛。
> 进度：已新增 `scripts/tui-smoke.sh` 和 `docs/tui-smoke.md`。脚本在 tmux 中捕获启动、首字符、清文本、宽字符、resize、`/help`、退出；跨终端记录表留给人工逐终端填充。

## 验收 / 测试（离线优先）
- 组件：用 `VirtualTerminal` 后端断言渲染输出与按键行为。
- fuzzy/autocomplete：表驱动单测。
- 主题/能力探测：注入 env 断言降级路径（NO_COLOR/dumb/truecolor）。
- TUI-7：脚本化记录，人工核对表。

## 本轮落地

- `pi-tui` 新增 theme、terminal image、Image component、SelectorDialog、ANSI-aware wrapping/truncation。
- Markdown 与 SelectList 接入组件 theme；新增 public API 覆盖测试。
- 新增 TUI-7 tmux smoke 脚本和人工核对文档。
- 已验证：`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace`、`git diff --check`、`bash -n scripts/tui-smoke.sh`、tmux smoke 通过。
