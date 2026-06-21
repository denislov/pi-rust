下面是一份我认为适合 `pi-tui` 的多阶段强化方案。核心原则是：**不推翻现有字符串/差分模型，不引入 ratatui，不一次性重写 Editor；用“内部结构化、外部兼容”的方式把复杂度逐层拆出来。**

推荐路线是 **strangler-style incremental refactor**：先建立内部新抽象，再让旧组件逐步迁过去。公开 API 尽量保持 `Component::render(width) -> Vec<String>`，这样 `pi-coding-agent` 和现有测试不会被一次性打穿。

**总体目标**

当前 `pi-tui` 最大的问题不是“没有 UI 框架”，而是几类逻辑混在一起：

- 样式：组件里到处手动拼 ANSI/OSC 8，reset 规则分散。
- 宽度：可见宽度、ANSI 跳过、wrap/truncate、cursor column 映射分散。
- paste marker：现在是字符串 marker + `HashMap`，但导航、删除、换行原子性还不够系统。
- autocomplete：provider、UI 状态、编辑器输入处理、Tab 行为耦合在 Editor 内。
- editor 状态：文本、光标、undo、history、kill ring、paste、autocomplete、render 都挤在一个大文件里。

强化方向应该是：**保留字符串输出作为边界，内部改成结构化模型；先清理基础设施，再拆 Editor。**

**阶段 0：建立行为基线和重构护栏**

先不改架构，补一批 characterization tests，把当前行为钉住。这个阶段的价值是避免后面“看似重构，实际改行为”。

重点测试：

- `visible_width` 对 CSI、OSC、OSC 8、APC、emoji、CJK、tab 的行为。
- Markdown 强调、链接、代码块、禁用颜色/启用颜色时的输出。
- Editor 的 cursor movement、history、paste marker submit expansion、undo、autocomplete Tab。
- `Tui` 的 inline differential 行为：不清屏、只更新 changed rows、cursor marker 定位。
- `pi-coding-agent` 的 scripted interactive smoke：输入、提交、Markdown 渲染、model/settings selector。

交付物：

- 新增 `tests/renderer_contract.rs`、`tests/editor_contract.rs` 或扩充现有测试。
- 明确哪些行为是 TS parity，哪些是 Rust 当前选择。
- 所有后续阶段必须保持 `cargo test -p pi-tui` 通过。

验收标准：

- 没有架构变更。
- 测试能覆盖后续要动的关键行为。
- 默认并发 workspace 测试里的已知环境竞态单独记录，不和 `pi-tui` 重构混在一起。

**阶段 1：引入结构化渲染行，但不改变公开 Component API**

不要立刻把 `Component::render` 改成返回结构化对象。先新增内部类型，让组件可以选择性使用：

```rust
pub struct RenderLine {
    segments: Vec<RenderSegment>,
}

pub enum RenderSegment {
    Text { text: String, style: Style },
    Hyperlink { text: String, url: String, style: Style },
    Raw(String),
}

pub struct RenderedLines(Vec<RenderLine>);
```

同时提供 materializer：

```rust
impl RenderLine {
    pub fn visible_width(&self) -> usize;
    pub fn truncate_to_width(&self, width: usize) -> RenderLine;
    pub fn to_ansi_string(&self, color: ColorLevel, hyperlinks: bool) -> String;
}
```

旧 API 仍然保持：

```rust
fn render(&mut self, width: usize) -> Vec<String>;
```

组件内部可以先用 `RenderLine` 构造，然后最后 `.to_ansi_string(...)`。这样不会影响 `Tui`、`pi-coding-agent` 和测试。

优先迁移组件：

- `Markdown`
- `Text`
- `TruncatedText`
- `SelectList`
- `SettingsList`
- `Loader`

暂时不要先动 `Editor`，因为 Editor 同时牵涉 cursor 映射和输入状态，风险更高。

收益：

- 样式和 OSC 8 不再散落在 Markdown/组件里。
- reset 规则统一。
- 禁用颜色、启用颜色、hyperlink capability 都有单点控制。
- 后续可以更安全地做宽度和 wrap。

验收标准：

- 公开 API 不破。
- Markdown 不再手写 OSC 8 字符串。
- `visible_width(line.to_ansi_string(...)) == line.visible_width()` 有测试守护。
- `cargo test -p pi-tui` 通过。

**阶段 2：统一宽度、截断、换行和 source map**

现在的宽度逻辑已经能用，但应该集中成一个“文本测量层”。建议新增模块：

```rust
src/text_layout/
  mod.rs
  width.rs
  wrap.rs
  truncate.rs
  source_map.rs
```

核心抽象：

```rust
pub struct VisualCellPos {
    pub byte_offset: usize,
    pub column: usize,
}

pub struct WrappedLine {
    pub source_start: usize,
    pub source_end: usize,
    pub visible_width: usize,
    pub text: String,
}
```

这里不要做成复杂排版引擎，目标很窄：

- 给定 `&str` 或 `RenderLine`，按 display width wrap。
- 不切 grapheme。
- 不把 ANSI/OSC 算进宽度。
- 返回 source offsets，方便 Editor 光标映射。
- 对前导空格、代码块缩进、paste marker 原子段预留能力。

这一步非常关键，因为 Editor 的“视觉行、逻辑位置、光标列、sticky column、paste marker 原子性”都依赖同一套映射。如果这层不统一，Editor 会继续越来越难维护。

交付物：

- `wrap_plain_text`
- `wrap_render_line`
- `truncate_render_line`
- `column_to_byte_offset`
- `byte_offset_to_column`
- ANSI/OSC 宽度测试集中到 `text_layout`。

验收标准：

- `Markdown`、`Text`、`TruncatedText` 使用新 layout。
- 旧 `utils::width` 可以继续 re-export，但内部转发到新模块。
- 代码块缩进、CJK、emoji、OSC 8 链接 wrap 都有测试。

**阶段 3：把 paste marker 从“字符串技巧”提升为 Editor 文档段**

当前 paste marker 是文本中的 marker 字符串加 `HashMap<usize, String>`。这能解决提交展开，但不够支撑 TS 里的原子导航和删除。建议引入 Editor 内部文档段模型：

```rust
pub enum EditorSegment {
    Text(String),
    PasteMarker {
        id: usize,
        marker: String,
        original: String,
    },
}

pub struct EditorDocument {
    segments: Vec<EditorSegment>,
}
```

但注意：不要马上把整个 Editor 改成 rope 或复杂文本结构。可以先提供一个 wrapper：

```rust
impl EditorDocument {
    pub fn display_text(&self) -> String;
    pub fn expanded_text(&self) -> String;
    pub fn insert_text(&mut self, cursor: DocCursor, text: &str);
    pub fn insert_paste(&mut self, cursor: DocCursor, original: String);
}
```

关键是引入 `DocCursor`：

```rust
pub struct DocCursor {
    segment_index: usize,
    offset: SegmentOffset,
}

pub enum SegmentOffset {
    TextByte(usize),
    BeforeMarker,
    AfterMarker,
}
```

这样 paste marker 可以成为不可进入的原子段。之后移动、删除、word navigation 都可以统一处理：

- Right 在 marker 前 -> marker 后。
- Left 在 marker 后 -> marker 前。
- Backspace 在 marker 后 -> 删除整个 marker。
- Delete 在 marker 前 -> 删除整个 marker。
- Word movement 把 marker 当一个 word-like token。
- Render wrap 把 marker 当原子显示段，必要时整段单独占行或按明确策略处理。

迁移策略：

1. 先只把大 paste 存成 `EditorSegment::PasteMarker`。
2. `Editor::text()` 仍返回 `display_text()`，保持外部兼容。
3. `Editor::expanded_text()` 返回 `expanded_text()`。
4. 后续再让 movement/delete 使用 segment-aware cursor。

验收标准：

- 手动输入 `[paste #99 +5 lines]` 不会被当成真实 paste。
- 真 paste marker 导航/删除是原子的。
- submit 展开原始内容。
- undo/redo 能恢复 paste marker 段，而不是只恢复字符串。

**阶段 4：拆 Editor 状态机为 Model / Commands / View**

这是最大阶段，必须在前面三阶段稳定后做。目标是把 [editor.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/components/editor.rs) 从“一个类包所有东西”拆成几个边界清晰的模块：

```text
src/components/editor/
  mod.rs
  component.rs        // 实现 Component，连接输入、model、view
  model.rs            // EditorModel: 文档、cursor、selection/scroll/history 状态
  command.rs          // EditorCommand + command dispatcher
  movement.rs         // grapheme/word/visual line movement
  edit_ops.rs         // insert/delete/kill/yank/undo/redo
  history.rs          // prompt history
  paste.rs            // paste marker/document integration
  view.rs             // render editor lines + cursor marker + border + scroll indicators
  autocomplete.rs     // editor-side autocomplete state
```

建议引入命令枚举：

```rust
pub enum EditorCommand {
    InsertText(String),
    Paste(String),
    DeleteBackward,
    DeleteForward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordLeft,
    MoveWordRight,
    Submit,
    Undo,
    Redo,
    JumpStart(JumpDirection),
    JumpTo(String),
    HistoryPrev,
    HistoryNext,
    AutocompleteTrigger { force: bool },
    AutocompleteAccept,
    AutocompleteCancel,
}
```

`handle_input` 只负责把 `InputEvent` 映射成 `EditorCommand`，不直接改文本。真正状态变更由 `EditorModel::apply(command)` 完成。

这样有几个直接收益：

- 输入协议和编辑行为分离。
- 后续支持自定义 keybindings 更容易。
- Undo/history/paste/autocomplete 不再互相穿透。
- 可以对 `EditorModel` 做大量无 UI 单元测试。
- `EditorView` 可以只测渲染，不测输入。

验收标准：

- `Editor` public API 基本不变。
- `EditorModel` 能独立测试。
- `handle_input` 的复杂度显著下降。
- 当前所有 editor tests 通过。
- 新增 command-level tests 覆盖关键行为。

**阶段 5：Autocomplete 改成独立 session 状态机**

当前 autocomplete 是同步 provider + Editor 内部状态。短期可用，但以后要追 TS 的 async/debounce/abort，就必须拆开。

建议新增：

```rust
pub struct AutocompleteSession {
    request_id: u64,
    mode: AutocompleteMode,
    prefix: String,
    items: Vec<AutocompleteItem>,
    selected: usize,
    status: AutocompleteStatus,
}

pub enum AutocompleteStatus {
    Idle,
    Pending,
    Ready,
    Cancelled,
}
```

Provider 分两层：

```rust
pub trait SyncAutocompleteProvider {
    fn get_suggestions(...) -> Option<AutocompleteSuggestions>;
}

pub trait AsyncAutocompleteProvider {
    fn request_suggestions(...) -> AutocompleteRequest;
}
```

不要一开始就把 async 接进 `Component::handle_input`。更稳的是：

- `Editor` 只管理 session。
- 外层 runtime 或 app loop 可以 poll/complete async request。
- 先保留 sync provider，内部适配成立即完成的 session。
- 未来再接 debounce/cancel/in-flight request id。

编辑器只关心这些事件：

```rust
EditorCommand::AutocompleteTrigger { force }
EditorCommand::AutocompleteUpdate { request_id, suggestions }
EditorCommand::AutocompleteAccept
EditorCommand::AutocompleteCancel
```

验收标准：

- 当前 slash completion 行为不变。
- Autocomplete state 不再散在 Editor 主体里。
- 支持“过期 request 结果丢弃”的测试，即使 provider 仍是同步 faux。
- 为后续 async provider 留接口，但不强行一次实现。

**阶段 6：把样式和主题能力探测集中化**

当 `RenderLine` 和 materializer 稳定后，把样式策略集中成 `RenderContext`：

```rust
pub struct RenderContext {
    pub color_level: ColorLevel,
    pub hyperlinks: bool,
    pub theme: TuiTheme,
}
```

组件渲染不要直接调用 `color_enabled()`，而是从 context/materializer 决定。不过为了保持 `Component::render(width)`，可以先让 `Tui` 或组件内部默认使用全局 context，未来再扩展 trait：

```rust
fn render_with_context(&mut self, width: usize, ctx: &RenderContext) -> Vec<String>;
```

过渡期可以保留默认实现：

```rust
fn render(&mut self, width: usize) -> Vec<String> {
    self.render_with_context(width, &RenderContext::default_from_env())
}
```

注意不要急着改 trait，因为这会影响 `pi-coding-agent` 和所有测试。可以先给组件内部使用，最后再考虑是否公开。

验收标准：

- Markdown hyperlink 是否启用由 context 控制。
- 颜色能力不在每个组件里散落调用。
- `TerminalCapabilities` 和 style/color 渲染有明确边界。
- disabled color 下输出纯文本，enabled color 下 ANSI 正确闭合。

**阶段 7：强化 Tui 差分模型，但不替换为 buffer UI**

现有 `Tui` 差分模型已经可用。未来可以强化，但不应改成 ratatui buffer。建议做这些小步：

1. 引入 `RenderFrame`：

   ```rust
   pub struct RenderFrame {
       pub lines: Vec<String>,
       pub cursor: Option<CursorPosition>,
       pub metadata: FrameMetadata,
   }
   ```

   现在 cursor marker 是先混入字符串再抽取。可以保留兼容，但内部逐步转成 frame metadata。

2. 差分逻辑独立模块化：

   ```text
   src/diff/
     line_diff.rs
     render_strategy.rs
   ```

3. overlay 合成独立成 frame compositor：

   ```text
   src/compositor.rs
   ```

4. 保留 `VirtualTerminal` 精确 op 测试。

这个阶段目标是让 `Tui` 从 700 多行降下来，而不是改变渲染语义。

验收标准：

- `Tui` 仍支持 `RenderSurface::Inline`。
- `RenderStrategy` 行为不变。
- cursor marker 字符串可逐步降级为兼容路径。
- `tui_render.rs` 现有测试继续通过。

**阶段 8：pi-coding-agent 适配层收敛**

最后处理 `pi-coding-agent` 对 `pi-tui` 的宽依赖。当前它直接用很多 `pi_tui::*` 类型，这是正常的，但后续可以收敛成一个小 adapter：

```text
pi-coding-agent/src/interactive/tui_adapter.rs
```

职责：

- 构造 `Tui<ProcessTerminal>`。
- 创建 `InteractiveRoot`。
- 处理 render scheduler。
- 暴露 scripted harness 使用的 `VirtualTerminal` 输出。
- 隔离将来 `pi-tui` 内部类型调整。

这不是为了隐藏所有东西，而是避免 app 层到处知道 terminal/render 细节。

验收标准：

- `interactive/app.rs` 中和 TUI runtime 相关的 glue 代码减少。
- scripted tests 仍能拿到 rendered lines、ops、cursor。
- 后续改 `RenderFrame` 或 context 不需要全文件搜索替换。

**建议优先级**

我建议按这个顺序做：

1. 阶段 0：测试护栏。
2. 阶段 1：`RenderLine` / `RenderSegment`。
3. 阶段 2：统一 width/wrap/truncate。
4. 阶段 3：paste marker 文档段。
5. 阶段 4：Editor Model/Command/View 拆分。
6. 阶段 5：Autocomplete session 状态机。
7. 阶段 6：RenderContext/style/theme 集中化。
8. 阶段 7：Tui frame/diff/compositor 内部整理。
9. 阶段 8：coding-agent adapter 收敛。

其中 **阶段 1、2、4 是主干**。
阶段 3 和 5 是为了补 TS parity 缺口。
阶段 7、8 是维护性收益，不必抢在前面。

**阶段切分成里程碑**

可以实际拆成 4 个开发里程碑：

| 里程碑 | 内容 | 风险 | 收益 |
|---|---|---:|---:|
| M1 Render Primitives | 阶段 0-2 | 中 | 高 |
| M2 Editor Document | 阶段 3 | 中高 | 高 |
| M3 Editor Architecture | 阶段 4-5 | 高 | 很高 |
| M4 Runtime Cleanup | 阶段 6-8 | 中 | 中高 |

**最小可落地版本**

如果只想先做一轮低风险改造，我建议只做：

- `RenderLine/RenderSegment`
- `text_layout` 模块
- Markdown/Text/SelectList 迁移
- 不动 Editor 主体

这轮就能立刻减少 ANSI/宽度问题，风险远低于直接拆 Editor。等这层稳定后，再进入 paste marker 和 Editor 状态拆分。

**不建议做的事**

- 不要把 `Component::render(width) -> Vec<String>` 立刻改掉。
- 不要一次性把 Editor 拆成十个文件后再补测试。
- 不要先引入 async autocomplete runtime。
- 不要把 paste marker 继续当普通字符串长期扩展。
- 不要让 `pi-coding-agent` 先适配新抽象，应该让 `pi-tui` 内部先兼容旧 API。
- 不要把 ratatui 的 `Buffer` 搬进来当半套依赖；这会形成两套渲染心智模型。

**推荐的第一张任务单**

第一阶段可以这样开工：

1. 新建 `src/render_line.rs`，定义 `RenderLine`、`RenderSegment`、materializer。
2. 给 `Style` 增加 materialize helper，不改变现有 `paint_with`。
3. 给 `RenderLine` 写 width/truncate/to_string 测试。
4. 迁移 `Markdown` 的 strong/link/code/span 输出到 `RenderLine`。
5. 迁移 `Text` 和 `TruncatedText`。
6. 保持所有 public exports 兼容。
7. 跑 `cargo test -p pi-tui --no-fail-fast`。
8. 再考虑 `SelectList` / `SettingsList`。

这条路能在不推翻现有模型的前提下，把未来最容易出错的基础层先打稳。