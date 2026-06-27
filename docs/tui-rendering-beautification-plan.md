# pi-rust 交互 TUI 渲染对标与美化方案

## 背景

本方案对标 TypeScript `pi` 的交互模式渲染表现，目标是改善当前 `pi-rust` 交互 TUI 的 transcript 可读性和状态辨识度，重点覆盖：

- thinking block 与普通助手消息的区分。
- tool 调用、执行中、成功、失败、结果预览的显示方式。
- 消息之间的留白节奏。
- 用户消息与助手消息、系统消息、工具消息的视觉层级。

本次只做方案整理，不修改运行代码。对标范围主要是：

- TS 交互组件：`pi/packages/coding-agent/src/modes/interactive/components/assistant-message.ts`
- TS 用户消息：`pi/packages/coding-agent/src/modes/interactive/components/user-message.ts`
- TS 工具消息：`pi/packages/coding-agent/src/modes/interactive/components/tool-execution.ts`
- TS bash 执行块：`pi/packages/coding-agent/src/modes/interactive/components/bash-execution.ts`
- Rust transcript 数据：`pi-rust/crates/pi-coding-agent/src/interactive/transcript.rs`
- Rust transcript 渲染：`pi-rust/crates/pi-coding-agent/src/interactive/render.rs`
- Rust 交互根组件：`pi-rust/crates/pi-coding-agent/src/interactive/root.rs`
- Rust theme token：`pi-rust/crates/pi-coding-agent/src/theme/tokens.rs`

## 对标结论

### 1. TS 的 transcript 是组件化视觉系统，Rust 当前是扁平文本渲染

TS 交互模式按消息类型使用不同组件：

- `UserMessageComponent`：用户消息放入带背景色的 `Box`，有内边距，文本使用 `userMessageText`，背景使用 `userMessageBg`。
- `AssistantMessageComponent`：助手正文不加背景，但正文前有留白，Markdown 左侧有 1 列缩进。
- `ToolExecutionComponent`：工具调用独立成块，有 pending/success/error 背景色，有内边距，并优先使用工具专用 renderer。
- `BashExecutionComponent`：bash 命令使用上下动态边框、命令标题、loader、输出预览、展开/折叠提示、错误/取消状态。
- `CustomMessageComponent`、`CompactionSummaryMessageComponent`：系统类或扩展类消息使用 `customMessageBg`、label、可折叠展示。

Rust 当前 `render_transcript_lines` 直接把 transcript item 映射成字符串：

- 用户消息渲染为 `user: <text>`。
- thinking 按黄色斜体逐行输出。
- 工具渲染为 `tool <name> <target> <status>`，结果直接跟在后面。
- 工具结束后遇到助手消息时插入一条整宽分隔线。
- 缺少 per-message 容器、背景、内边距、明确的状态块和工具专用展示。

直接结果是：Rust 虽然能用，但视觉层级不够明确，消息之间密度高，用户输入、助手输出、thinking 和工具输出容易混在一起。

### 2. Rust 已经具备美化所需的大部分底层能力

Rust 侧不是缺主题能力，而是 transcript 渲染没有充分使用它们。

已存在能力：

- `pi-tui::Box` 支持 padding 和背景回调。
- `pi-tui::Markdown` 支持主题化 Markdown 渲染。
- `pi-tui::Loader` 支持 spinner 文本。
- `Style` 支持 fg/bg/bold/dim/italic/underline 等样式。
- `ResolvedTheme` 已能解析 TS 对齐的 51 个 theme token。
- theme token 已包含 `userMessageBg`、`userMessageText`、`toolPendingBg`、`toolSuccessBg`、`toolErrorBg`、`toolTitle`、`toolOutput`、`thinkingText`、`customMessageBg`、`customMessageText` 等。

当前主要缺口是 `render_transcript_lines` 没有拿到 `ResolvedTheme`，也没有抽象出 transcript block renderer。

### 3. thinking block 的当前问题

TS 行为：

- thinking 与普通正文同属 assistant message，但用 `thinkingText` 颜色和 italic 显示。
- 可通过 `hideThinkingBlock` 隐藏真实 thinking，显示一个静态 `Thinking...` label。
- thinking 与后续正文之间按内容关系插入空行，避免不必要的尾部留白。

Rust 当前行为：

- thinking 直接用硬编码黄色斜体输出。
- 没有 label、左侧标识、块边界或缩进。
- `hide_thinking_block` 为 true 时直接不显示，没有 TS 风格的隐藏提示。
- thinking 后接正文时区分度弱，视觉上像普通助手正文的一部分。

结论：应优先使用 `thinkingText` token，并加轻量块结构。只靠颜色不足以解决“thinking block 跟其他消息不好区分”的问题。

### 4. tool 显示的当前问题

TS 行为：

- tool 有统一 shell：pending/success/error 三种背景。
- tool title 使用 `toolTitle`，输出使用 `toolOutput`。
- 内置工具各自有展示格式：
  - `read`：`read <path>`，可带行范围，展开时显示更多内容。
  - `write`：`write <path>`，展示写入内容或结果摘要。
  - `edit`：自渲染 diff，避免普通背景盒吞掉 diff 的语义颜色。
  - `bash`：`$ <command>`，输出尾部预览，支持取消、exit code、truncation、展开/折叠。
  - `grep`、`find`、`ls`：展示 pattern/path/limit 和结果预览。
- 自定义工具可覆盖 `renderCall` / `renderResult`，并可选择 `renderShell: "self"`。

Rust 当前行为：

- 所有工具走同一行 header：`tool <name> <target> <status>`。
- 状态只用 `running/error/done` 文本和颜色，没有背景块。
- args 只抽取一个 target，复杂参数不可读。
- result 只是按行输出，默认最多 3 行，展开最多 20 行。
- `bash` 没有专用边框、loader、exit/truncation/取消视觉语义。
- `edit/write/read` 没有专用摘要或 diff 友好展示。

结论：tool 是最影响质感的部分，应分成“统一工具块 shell”和“内置工具 renderer”两层推进。

### 5. 消息间距的当前问题

TS 的消息节奏是“块之间留白、块内紧凑”：

- 用户消息前通常插入 `Spacer(1)`。
- 工具块和 bash 块自带顶部留白。
- 助手正文有首部留白，但避免 tool 前后重复空行。
- 系统状态消息连续出现时会复用上一行，避免刷屏。

Rust 当前是 transcript item 顺序直出：

- 多数 item 之间没有统一空行策略。
- 工具结果、助手正文、用户消息密集堆叠。
- 额外的整宽 `─` 分隔线只在“已完成工具后接助手”时出现，规则突兀且与 TS 不一致。

结论：应采用统一 spacing policy，而不是在某个相邻关系上临时插线。

## 目标视觉规范

### 总体原则

1. 块之间留 1 行，块内保持紧凑。
2. 用户消息使用背景盒，助手正文保持无背景。
3. thinking 使用低强调独立块：同 assistant 语义，但有左侧标识、缩进、`thinkingText`、italic。
4. tool 使用状态背景：pending/success/error 一眼可见。
5. 工具输出默认给摘要，展开后给更多，但不能让 transcript 被长输出淹没。
6. 所有样式优先来自已移植的 theme token，不新增硬编码色。
7. 窄屏优先保证不溢出，文本可截断或换行，但不破坏 ANSI 宽度。

### 推荐视觉草图

用户消息：

```text

  你能帮我检查 src/lib.rs 吗？

```

说明：实际渲染应有 `userMessageBg` 背景和左右/上下 padding，而不是字面边框。保持 TS `UserMessageComponent` 的体验。

助手正文：

```text

  我先看一下相关文件，然后给出结论。
```

thinking 可见时：

```text

  thinking
    需要先定位入口文件，再确认测试覆盖。
```

thinking 隐藏时：

```text

  Thinking...
```

工具 pending：

```text

  read src/lib.rs
```

工具 success：

```text

  read src/lib.rs

  pub fn run(...) { ... }
  ... 17 more lines (Ctrl+P to expand)
```

bash：

```text

────────────────────────────────────────
  $ cargo test -p pi-coding-agent

  test interactive_mode ... ok
  test interactive_transcript ... ok

  ... 42 more lines (Ctrl+P to expand)
────────────────────────────────────────
```

注：以上草图展示结构，实际颜色由 theme token 控制。

## 方案选择

### 方案 A：最小补丁，只改 `render_transcript_lines`

内容：

- 给用户消息加背景盒。
- thinking 使用 `thinkingText` 和 label。
- tool 加背景色和简单缩进。
- 增加统一空行策略。

优点：

- 改动小，见效快。
- 不需要修改 transcript 数据结构。

缺点：

- 工具专用 renderer 很难做干净。
- bash/edit/read 的 TS parity 只能部分实现。
- `render_transcript_lines` 会继续膨胀，后续维护差。

### 方案 B：引入 transcript block renderer，分阶段移植 TS 视觉语义

内容：

- 保留 `TranscriptItem` 作为数据层。
- 新增 `TranscriptRenderTheme` / `TranscriptRenderOptions`。
- 将用户、助手、thinking、tool、system/error 拆成独立 renderer 函数或小组件。
- 第一阶段先完成块结构和 theme token；第二阶段补内置工具 renderer；第三阶段补 bash/diff/图片/自定义 renderer parity。

优点：

- 与 TS 组件职责更接近，但仍保持 Rust 简洁。
- 可逐步测试，每类消息都有独立快照/单测。
- 后续可接入 `ResolvedTheme`、工具 details、图片等更自然。

缺点：

- 初始改动比方案 A 大。
- 需要设计少量中间结构，避免过度抽象。

### 方案 C：完整复刻 TS component tree

内容：

- 为 Rust 交互 transcript 建立接近 TS 的组件树。
- 每个消息都是 `Component`，由 root 管理 children。
- 工具、assistant、user、custom 全部组件化。

优点：

- parity 最强。
- 动态更新、局部 invalidate、扩展 renderer 更自然。

缺点：

- 改造范围最大。
- 当前 Rust transcript 与滚动逻辑是按 `Vec<String>` 渲染，整体换模型风险高。
- 对现阶段目标过重。

推荐选择：方案 B。它能解决当前观感问题，同时避免一次性重写交互架构。

## 详细美化方案

### 阶段 1：消息块层级和主题接入

目标：先解决用户反馈最明显的问题：用户消息不突出、thinking 不好区分、消息太密。

改造点：

1. 修改 `InteractiveRoot::render` 调用链，让 `render_transcript_lines` 能拿到 `ResolvedTheme` 或一个从 `ResolvedTheme` 派生的轻量 `TranscriptRenderPalette`。
2. 新增 theme helper：
   - `fg_token(theme, ThemeColor) -> Style`
   - `bg_token(theme, ThemeBg) -> Style`
   - 或更具体的 `TranscriptStyles`，避免 render 函数到处直接处理 token。
3. 用户消息改为 Box 风格：
   - 背景：`ThemeBg::UserMessageBg`
   - 文本：`ThemeColor::UserMessageText`
   - padding：左右 1，上下 1
   - Markdown：保持 `preserveOrderedListMarkers` 等 TS 行为的 Rust 等价能力；若 Rust Markdown 暂不支持该 option，先记录测试缺口。
4. 助手正文：
   - 保持无背景。
   - 正文左缩进 1。
   - 首个可见 assistant block 前插入 1 行空行。
5. thinking block：
   - 可见时使用 `ThemeColor::ThinkingText` + italic。
   - 加 `thinking` label 或左侧弱标识，内容缩进 2。
   - 与后续正文之间仅在确实有后续可见内容时插入空行。
   - 隐藏时显示 `Thinking...` 或配置里的 hidden label，而不是完全消失。
6. 系统/错误消息：
   - 系统消息使用 `dim`/`muted` 风格，避免和 assistant 正文同权重。
   - 错误消息使用 `error`，前面保留 `Error:` label。

验收：

- 同一屏内能一眼区分 user、assistant、thinking、tool。
- 无颜色终端下仍能靠缩进、label、留白区分。
- 所有行 `visible_width <= width`。

### 阶段 2：统一工具块 shell

目标：让所有工具调用都有稳定、友好的状态视觉。

改造点：

1. 为工具渲染引入统一结构：
   - header：工具名 + target/summary + status。
   - body：参数摘要或结果预览。
   - footer：截断/展开/错误提示。
2. 状态背景：
   - pending：`ThemeBg::ToolPendingBg`
   - success：`ThemeBg::ToolSuccessBg`
   - error：`ThemeBg::ToolErrorBg`
3. 状态文本：
   - pending：`running` 或 spinner frame；若事件层没有持续 tick，先用 `running`。
   - success：`done`
   - error：`error`
4. 文本 token：
   - 工具标题：`ThemeColor::ToolTitle` + bold。
   - 工具输出：`ThemeColor::ToolOutput`。
   - 错误输出：`ThemeColor::Error` 或 `ToolError` 现有 style。
5. 默认折叠策略：
   - collapsed：最多 3 行结果，与当前 Rust 行数一致，但加 `... N more lines (expand tools)`。
   - expanded：最多 20 行，与当前 Rust `EXPANDED_TOOL_RESULT_LINES` 一致。
   - write/edit 可继续允许完整关键内容，但要有独立规则，避免大文件写入淹没屏幕。
6. 删除或弱化当前“工具完成后接助手插入整宽分隔线”的特殊规则，改由统一块间距承担分隔。

验收：

- pending/success/error 只看背景就能区分。
- collapsed/expanded 输出都不超宽。
- 工具之间、工具与助手正文之间留白稳定。

### 阶段 3：内置工具 renderer parity

目标：把 TS 中最有信息密度的工具摘要移植到 Rust。

优先级：

1. `read`
   - header：`read <path>`，支持 `file_path` / `path`，显示行范围。
   - result：代码/文本预览，保持 tab 替换和行数截断。
   - 后续可接 syntax highlight。
2. `bash`
   - header：`$ <command>`。
   - 运行中显示 loader 或 `Running...`。
   - 成功/失败显示 exit code、cancelled、truncation。
   - collapsed 显示尾部输出，而不是头部输出。
3. `edit`
   - 使用 self-render 风格，不套普通工具背景吞掉 diff 语义。
   - 显示文件路径、首个变更行、diff added/removed/context 颜色。
4. `write`
   - header：`write <path>`。
   - 显示写入摘要，不默认展开完整大文件内容。
5. `grep` / `find` / `ls`
   - header 展示 pattern/path/glob/limit。
   - result 使用路径友好的列表预览。

需要的数据改造：

- 当前 `TranscriptItem::Tool` 只有 `args`、`result: Option<String>`、`is_error`。
- 若要完整对齐 TS，需要保留：
  - partial/final 状态。
  - tool result details。
  - content block 类型，尤其 image content。
  - bash exit code、truncation、full output path。
- 建议先保持现有字段做文本 renderer parity；后续再扩 `ToolDisplayResult`。

验收：

- 常用内置工具的 header 不再像通用日志。
- bash 输出默认显示尾部，长输出有清晰展开提示。
- edit diff 的新增/删除/上下文有独立颜色。

### 阶段 4：可扩展渲染与高级内容

目标：为后续 TS parity 留接口，但不阻塞第一轮美化。

可选项：

1. 自定义工具 renderer
   - Rust 可定义 `ToolRenderer` trait：
     - `render_call(args, context) -> Vec<String>`
     - `render_result(result, options, context) -> Vec<String>`
     - `render_shell() -> Default | SelfRendered`
   - 内置工具先用 trait 实现。
   - 外部扩展接入等插件系统成熟后再公开。
2. 图片结果
   - 复用 `pi-tui::Image` 和 terminal image 能力。
   - 无图像能力或关闭图片时，显示 image fallback。
3. OSC 133 shell integration
   - TS 用户/助手消息会包 OSC 133 zone。
   - Rust 后续可在 transcript block 层加 zone 包裹，先不作为美化首要项。
4. 超长 thinking
   - 可折叠 thinking，或隐藏时保留一行 `Thinking...`。
   - 避免 reasoning 大段内容压缩上下文视野。

## 实现建议

### 新增或调整的数据结构

建议新增：

```rust
struct TranscriptRenderOptions<'a> {
    width: usize,
    max_tool_result_lines: usize,
    color: bool,
    markdown_theme: &'a pi_tui::MarkdownTheme,
    hide_thinking_block: bool,
    hidden_thinking_label: &'a str,
    tool_output_expanded: bool,
}

struct TranscriptStyles {
    user_text: Style,
    user_bg: Style,
    thinking: Style,
    system: Style,
    error: Style,
    tool_title: Style,
    tool_output: Style,
    tool_pending_bg: Style,
    tool_success_bg: Style,
    tool_error_bg: Style,
}
```

如果 `Style` 的背景和前景组合不够方便，可新增小 helper 生成 `Style { fg, bg, ... }`。

### 函数拆分

建议将 `render.rs` 拆成清晰的私有函数：

- `render_user_message`
- `render_assistant_message`
- `render_thinking_block`
- `render_tool_block`
- `render_tool_header`
- `render_tool_result_preview`
- `render_system_message`
- `render_error_message`
- `apply_block_spacing`

后续如果文件变大，再拆成：

- `interactive/render/mod.rs`
- `interactive/render/transcript.rs`
- `interactive/render/tools.rs`
- `interactive/render/styles.rs`

第一轮可以先在单文件内完成，避免过早重构。

### spacing policy

推荐规则：

1. transcript 第一个 item 前不强制空行，除非它是用户/工具/助手可见块且欢迎系统消息已经存在。
2. `User`、`Assistant`、`Tool`、`Custom/System notice` 这些块之间插入 1 行空行。
3. 同一个 assistant 内部：
   - thinking 与正文之间按 TS 规则，仅在后续有可见内容时插入 1 行。
   - 多段 Markdown block 交给 Markdown renderer 自己处理。
4. 连续系统状态消息可以未来合并；第一轮只保证 dim 和留白。
5. 不再依赖整宽 `─` 作为主要分隔手段。

### 宽度处理

必须坚持：

- 所有输出行走 `fit_line` 或 ANSI-aware wrapping/truncation。
- 背景行要填满可见宽度还是只包裹内容，需要统一：
  - 用户消息和工具块建议背景填满当前内容行的 padding 区域，不强制铺满整屏。
  - 若 `Box` 当前会 pad 到 width，可直接复用；否则新增局部 helper。
- 窄屏下 header 优先保留工具名和状态，target 可 ellipsis。

## 测试计划

### Rust 单元测试

新增或扩展：

- `pi-rust/crates/pi-coding-agent/tests/interactive_transcript.rs`
  - user 消息渲染包含内容且不再是裸 `user:`。
  - hidden thinking 显示 `Thinking...`。
  - visible thinking 使用独立 label/缩进。
  - tool pending/success/error 各有不同状态文本。
  - collapsed tool result 显示省略提示。
- `pi-rust/crates/pi-coding-agent/tests/interactive_mode.rs`
  - scripted prompt 下用户消息、助手消息、工具消息都可见。
  - hide thinking setting 生效。
  - tool expand key 影响最大展示行数。
- `pi-rust/crates/pi-tui/tests/components.rs`
  - 若需要增强 `Box` 或 background helper，在 pi-tui 层补宽度测试。

### 宽度与无色终端测试

每个新增渲染测试都应覆盖：

- `color = true`
- `color = false`
- `width = 40`
- 一个窄屏宽度，例如 `width = 20`

断言每行：

```rust
assert!(pi_tui::visible_width(line) <= width);
```

### 人工 smoke

更新 `pi-rust/docs/tui-smoke.md` 或新增记录项：

- 输入普通问题，看用户消息是否突出。
- 使用会触发 thinking 的 reasoning model，看 thinking 是否独立。
- 触发 `read`、`bash`、`edit`、`grep`、`find`、`ls`。
- 切换展开工具输出。
- 切换 hide thinking。
- 终端窄宽度下检查没有溢出和重叠。

## 分阶段交付建议

### P0：快速可见改善

范围：

- `ResolvedTheme` 接入 transcript render。
- 用户消息 Box 化。
- thinking token 化、label 化、隐藏时显示提示。
- 统一块间空行。
- 工具块 pending/success/error 背景。

预期收益：

- 直接解决“用户消息不好区分”“thinking block 不好区分”“消息太密集”。

建议验证：

- `cargo fmt --check`
- `cargo test -p pi-coding-agent interactive`
- `cargo test -p pi-tui markdown components`

### P1：工具展示质量

范围：

- `read`、`bash`、`edit` 三个高频工具 renderer。
- collapsed/expanded 文案和截断提示。
- bash tail preview、exit/cancel/truncation 状态。

预期收益：

- 直接解决“tool 工具调用显示不友好美观”。

建议验证：

- `cargo test -p pi-coding-agent tool`
- `cargo test -p pi-coding-agent interactive`

### P2：完整工具 parity 与高级内容

范围：

- `write`、`grep`、`find`、`ls` renderer。
- diff 细化。
- image fallback 或 image block。
- 为未来 extension renderer 留 trait。

预期收益：

- 接近 TS pi 的长期可维护视觉体系。

建议验证：

- `cargo test --workspace`
- `scripts/tui-smoke.sh`

## 风险与注意事项

1. 不要把所有 TS component 机械搬进 Rust。
   - 当前 Rust 的 transcript 是数据驱动字符串渲染，短期用 block renderer 更稳。
2. 不要只靠颜色区分。
   - 无色终端、低对比主题、截图场景下仍要靠 spacing、label、缩进。
3. 工具 renderer 不要一次性追求完整扩展系统。
   - 先内置 renderer parity，再考虑公开 trait。
4. 避免长输出影响主对话。
   - 默认 collapsed，展开受全局 `tool_output_expanded` 控制。
5. 保持滚动语义。
   - transcript 行数变化会影响 `scroll_offset`，新增 spacing 后要检查 scrolled view 锁定逻辑。
6. 不要破坏 Markdown 宽度。
   - 背景、缩进、ANSI 样式都必须经过 visible-width 测试。

## 推荐落地顺序

1. 建立 `TranscriptRenderOptions` 和 `TranscriptStyles`，把 `ResolvedTheme` 传入 transcript render。
2. 重写 user/assistant/thinking 的渲染，补单测。
3. 增加统一 spacing policy，移除突兀的 tool-to-assistant 分隔线规则，补滚动/宽度测试。
4. 给 tool 加统一背景 shell 和状态样式，保留现有 result 行数规则。
5. 移植 `read`、`bash`、`edit` renderer。
6. 移植 `write`、`grep`、`find`、`ls` renderer。
7. 更新 smoke 文档，跑脚本化和人工窄屏检查。

## 最终验收标准

- 用户消息在 transcript 中具有独立背景块，不再只是 `user:` 前缀。
- thinking block 与助手正文有清晰区分；隐藏 thinking 时仍有可见状态提示。
- tool 调用有 pending/success/error 三态背景，常用工具 header 可读。
- bash 输出具备命令标题、运行状态、尾部预览、错误/取消/截断提示。
- 消息之间留白稳定，同屏扫描不会显得拥挤。
- 无色终端下仍能靠 label、缩进和留白读懂结构。
- 所有新增渲染路径都有宽度测试，任何行都不超过 render width。
- 不改变底层 agent 行为，不影响会话持久化格式，除非后续阶段明确扩展 tool result details。
