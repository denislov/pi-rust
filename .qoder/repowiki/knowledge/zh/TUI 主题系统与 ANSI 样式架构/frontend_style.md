该仓库的前端样式体系完全基于 **Rust TUI（终端用户界面）**，不依赖 Web 前端技术（如 CSS、HTML 或 Tailwind）。其核心是一个位于 `pi-tui` crate 中的自定义样式引擎，通过 ANSI 转义序列在终端中实现视觉一致性。

### 1. 核心系统与方法论
- **ANSI 样式引擎**：`pi-tui/src/style.rs` 定义了底层的 `Color`（支持 ANSI 16/256 及 TrueColor RGB）和 `Style`（前景、背景、粗体、暗淡、反转）结构。通过 `paint_with_level` 函数将样式转换为 ANSI SGR 序列，并根据环境变量（如 `COLORTERM`, `TERM_PROGRAM`）自动检测终端的颜色支持等级。
- **结构化主题（Theming）**：`pi-tui/src/theme.rs` 实现了组件化的主题系统。它定义了 `TuiTheme`，包含 `ThemePalette`（全局调色板）以及针对特定组件的子主题（如 `MarkdownTheme`, `EditorTheme`, `SelectListTheme`）。
- **预设模式**：系统内置了 `dark()` 和 `light()` 两种标准主题模式，并支持通过 `custom()` 方法创建自定义调色板。

### 2. 关键文件与包
- **`crates/pi-tui/src/theme.rs`**：定义所有 UI 组件的视觉契约，包括 Markdown 渲染、编辑器边框、选择列表高亮等样式规则。
- **`crates/pi-tui/src/style.rs`**：提供底层的颜色映射、样式组合逻辑以及环境驱动的颜色能力探测。
- **`crates/pi-tui/src/components/markdown.rs`**：展示如何将 `MarkdownTheme` 应用于 `pulldown-cmark` 解析出的事件流，实现代码块、链接和引用的差异化着色。
- **`crates/pi-coding-agent/src/interactive/app.rs`**：作为主应用入口，负责初始化 `TuiTheme` 并将其注入到交互式会话中。

### 3. 架构约定
- **组件驱动样式**：每个 UI 组件（如 `Editor`, `Markdown`, `SelectList`）都拥有独立的 `Theme` 结构体，通过 `TuiTheme` 统一分发。这种设计确保了样式的模块化，避免了全局样式污染。
- **语义化颜色常量**：在 `style.rs` 中定义了如 `USER` (Cyan), `ERROR` (Red/Bold), `TOOL_NAME` (Yellow) 等语义化常量，确保跨模块的视觉语义一致。
- **响应式策略**：由于是 TUI 应用，"响应式"主要体现在对终端窗口大小变化的监听（通过 `crossterm`），组件根据实时提供的 `width` 和 `height` 进行重排和截断（如 `truncate_to_width`）。

### 4. 开发者规范
- **禁止硬编码颜色**：在开发新组件时，必须从 `theme` 结构中获取 `Style`，严禁直接使用 `Color::Red` 等硬编码值，以保障主题切换的有效性。
- **样式继承**：新组件应遵循现有的 `Theme` 命名规范（如 `XxxTheme`），并在 `TuiTheme` 中注册对应的字段。
- **终端兼容性**：所有样式输出必须经过 `paint_with` 或类似工具函数处理，以确保在不支持颜色的终端（`NO_COLOR=1`）中能优雅降级为纯文本。