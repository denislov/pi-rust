# M11 — 交互体验补全

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)、[M9](M9-agent-harness.md)、[M10](M10-resources-input.md)
> 定位：**交互模式从"能跑"到"好用"**。含交互发布门 TUI-7。
> **状态：可复用 TUI/交互基础设施已就位**。slash command registry、pi-tui 组件库、主题系统、fuzzy/autocomplete、终端图像协议均已落地。
> 剩余项为产品化级别的业务选择器接线和 M13 外设类命令真实后端。

## 目标
补齐 slash 命令、交互组件、`pi-tui` 组件库与主题系统，让交互模式达到 pi 的体验水准。

## 实际推进状态

M11 已按可离线验证的交互基础设施路径推进完成。当前落地范围：

- slash command registry 已覆盖 TS built-in 命令名（23 个），`/help`、`/quit`、`/name`、`/session`、`/hotkeys`、`/model`、`/copy`、`/export`、`/import`、`/clone`、`/new`、`/reload` 具备本地行为；其余依赖外设/会话后端的命令返回显式未实现提示，留给对应里程碑接管后端行为。
- `pi-tui` 已具备可复用 `Loader` / `CancellableLoader`、`Box`、`TruncatedText`、`SettingsList`、`Image`、`SelectorDialog` 等组件基础。
- fuzzy 搜索和同步 autocomplete 已移植，覆盖 slash command、路径/附件、环境变量建议。
- 主题系统已落地 dark/light/custom palette，并为 Markdown、SelectList、SettingsList、Editor 提供 theme 结构；Markdown 与 SelectList 已接入组件 theme 参数。
- 高级文本处理新增 ANSI-aware wrapping 与 ellipsis truncation，保留 SGR 序列并按 grapheme / wide width 控制行宽。
- 终端图像协议新增 Kitty / iTerm2 编码、删除序列、能力探测、PNG/JPEG/GIF/WebP 尺寸解析、cell size 计算和 `Image` fallback。
- TUI-7 smoke suite 已落地 tmux 脚本和人工记录文档。

## 已实现项

### 1. slash 命令（23 个）
- ✅ 内置 registry 覆盖 23 命令名（对照 TS 的 21 个 built-in）。
- ✅ `/help`、`/quit`、`/name`、`/session`、`/hotkeys`、`/model`、`/copy`、`/export`、`/import`、`/clone`、`/new`、`/reload` 已有本地行为。
- ⏭️ 其余会话/外设命令（`/share`、`/login`、`/logout`、`/fork`、`/tree`、完整 HTML 导出 parity 等）的后端实现归 M13/后续交互 polish。

### 2. 交互组件
- ✅ 通用 `SelectorDialog` 已实现，`/model` selector 已有基础状态。
- ✅ selector/dialog 基础 API 覆盖标题、help、fuzzy SelectList、confirm/cancel callback。
- ⏭️ 具体业务对话框（ModelSelector、SessionSelector、ScopedModelsSelector、ThemeSelector 等）待后续接入。

### 3. `pi-tui` 组件库
- ✅ `Loader` / `CancellableLoader`：支持 frame tick、消息更新、宽度裁剪、Escape/Ctrl+C 取消。
- ✅ `Box`：padding + 背景回调。
- ✅ `TruncatedText`：单行截断、宽度约束。
- ✅ `SettingsList`：键盘导航、fuzzy 搜索、值循环、change/cancel 回调。
- ✅ `Image`：Kitty/iTerm2 协议 + fallback 文本。
- ✅ coding-agent interactive footer spinner 已迁移为复用 `pi_tui::Loader`。
- ⏭️ SettingsList submenu 工厂待 selector/dialog 栈补齐后接入。

### 4. 搜索与补全
- ✅ TS-parity fuzzy scoring/filtering，`SelectList` 已切到 fuzzy 匹配。
- ✅ 同步 `CombinedAutocompleteProvider`，支持 slash command、路径/`@` 附件、`$ENV` 建议。

### 5. 主题系统
- ✅ ANSI 256/RGB SGR 输出，颜色能力探测（NO_COLOR、dumb、truecolor/24bit、*-256color）。
- ✅ dark/light/custom theme 对象，Markdown/SelectList 已接收 theme 参数。
- ⏭️ SettingsList/Editor theme 接入待后续。

### 6. 高级文本处理
- ✅ `wrap_text_with_ansi` + `truncate_to_width_with_ellipsis`：ANSI SGR 保留、grapheme、wide emoji。
- ⏭️ emoji 探测缓存和 word-navigation 细化（对标 `Intl.Segmenter`）可作为后续 polish。

### 7. 终端图像协议
- ✅ `terminal_image` 模块：Kitty/iTerm2 编码、删除序列、能力探测、PNG/JPEG/GIF/WebP 尺寸解析。
- ✅ `Image` 组件：可用时渲染协议序列，否则 fallback 文本。

### 8. TUI-7 smoke suite
- ✅ `scripts/tui-smoke.sh` + `docs/tui-smoke.md`。
- ⏭️ 跨终端人工记录表待逐终端填充。

## 已知缺口
- Editor 字符跳转：键位定义存在但 handler 未实现。
- Editor prompt 历史：上下箭头浏览历史缺失。
- Editor autocomplete 集成：`CombinedAutocompleteProvider` 存在但 editor 未接入。
- Paste markers：大段粘贴标记替换缺失。
- Editor scroll offset + border：Rust 无视口滚动和主题化边框。
- OSC 8 hyperlinks：Markdown 链接不可点击。
- Bold markdown：未从 pulldown-cmark bold 事件解析。
- 业务选择器：ModelSelector、SessionSelector、ScopedModelsSelector、ThemeSelector 等 TUI 对话框待接入。

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
