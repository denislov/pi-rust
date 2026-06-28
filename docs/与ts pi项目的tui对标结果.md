**总体结论**

`pi-rust/crates/pi-tui` 是目前 Rust 迁移里和 TS parity 最接近的 crate。它不是空壳，也不是只做了少量组件，而是已经覆盖了 TS `pi/packages/tui` 的大部分核心能力：组件体系、TUI 渲染、terminal 抽象、keyboard parsing、stdin buffer、keybindings、overlay、editor、autocomplete、markdown、terminal image、terminal color、fuzzy、width/ANSI 工具，以及测试用 `VirtualTerminal`。

粗略完整度判断：

| 维度 | Rust 当前完整度 | 判断 |
|---|---:|---|
| 公共导出面 | 75%-85% | 主要模块都对应上了，Rust 还多了 `SelectorDialog`、`VirtualTerminal`、`RenderScheduler`、style/theme facade |
| TUI 渲染核心 | 70%-80% | differential/full redraw、cursor marker、overlay、Kitty image cleanup 都有；TS 的 async render scheduling/start-stop 集成更完整 |
| Terminal/input/keybindings | 75%-85% | Kitty/legacy CSI/Windows VT/modifyOtherKeys/terminal progress 都有；TS 的 stdin callback 接线和 native modifier 支持更完整 |
| Editor | 65%-75% | undo/redo、kill ring、history、paste、autocomplete、word navigation 都有；TS 的异步 autocomplete、paste marker 原子分段、TUI 联动更成熟 |
| Autocomplete | 45%-60% | Rust 有 slash/file/env 补全；TS 有 `fd` 异步全树搜索、AbortSignal、gitignore/hidden/exclude 策略 |
| Markdown | 65%-75% | Rust 用 `pulldown-cmark` 覆盖主要渲染；TS 用 `marked` 加自定义 tokenizer，细节 parity 不完全 |
| Terminal image | 65%-75% | Kitty/iTerm2 编码、尺寸解析、capability detection 有；TS 的全局 capability/cache/cell-size/image id API 更完整 |
| 公共接口稳定性 | 55%-65% | public API 测试存在，但模块全 public、结构体字段裸露较多、没有稳定 facade 或 `non_exhaustive` 策略 |

**功能完整度**

TS `@earendil-works/pi-tui` 的导出面集中在 `src/index.ts`，包括 autocomplete、components、keybindings、keys、stdin buffer、terminal、terminal colors、terminal image、TUI/overlay 和 utils，见 [pi/packages/tui/src/index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/index.ts:3)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/index.ts:34)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/index.ts:71)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/index.ts:99)。

Rust `pi-tui` 的 `lib.rs` 基本按同样结构导出：autocomplete、component、components、cursor、fuzzy、input、overlay、runtime、style、terminal、terminal_colors、terminal_image、theme、tui、utils、virtual_terminal、word_navigation，见 [pi-rust/crates/pi-tui/src/lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/lib.rs:1) 和 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/lib.rs:21)。这说明 Rust 侧功能面不是“按需零散补”，而是有系统性映射。

核心 TUI 运行时方面，TS `Component` 是简单对象接口，`render(width)`、可选 `handleInput(data)`、`wantsKeyRelease`、`invalidate()`，见 [pi/packages/tui/src/tui.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/tui.ts:61)。Rust 改成 `Component` trait，`render(&mut self)`、`handle_input(&InputEvent)`、focus setter、viewport setter、downcast helper，见 [pi-rust/crates/pi-tui/src/component.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/component.rs:3)。这是合理的 Rust 化，但不是 TypeScript API 的逐字复刻。

Rust `Tui` 已经实现了 children、overlay、focus、input listener、terminal color listener、render strategy、cursor marker、Kitty image cleanup、full/differential render，见 [pi-rust/crates/pi-tui/src/tui.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/tui.rs:18)、[tui.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/tui.rs:71)、[tui.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/tui.rs:468)。TS `TUI` 更像完整 event loop 入口，包含 `start()`、`stop()`、`requestRender()` 节流、debug key、terminal color notification、cell size query 和复杂 overlay focus restore，见 [pi/packages/tui/src/tui.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/tui.ts:300)、[tui.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/tui.ts:489)。Rust 目前更偏“可被上层 loop 驱动的渲染器”，而 TS 是“自己接管 terminal lifecycle 的 UI runtime”。

Overlay 是一个明显差异点。TS 的 `OverlayHandle` 是闭包式对象，`hide/setHidden/focus/unfocus/isFocused` 都能直接操作原 TUI 实例，见 [pi/packages/tui/src/tui.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/tui.ts:215)。Rust 的 `OverlayHandle` 是 ID token，方法需要传入 `&mut Tui<T>`，见 [pi-rust/crates/pi-tui/src/overlay.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/overlay.rs:103)。这是 Rust 所有权下的自然设计，但 TS 那套“overlay focus restore blocked/eligible 状态机”更复杂，Rust 目前只是 `restore_focus: Option<ComponentId>`，见 [overlay.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/overlay.rs:126) 对比 [tui.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/tui.ts:370)。

Terminal 方面，TS `Terminal` 的 `start(onInput, onResize)` 负责接线，`drainInput` 是 Promise，直接暴露 columns/rows getter，见 [pi/packages/tui/src/terminal.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/terminal.ts:52)。Rust `Terminal` trait 更底层，同步返回 `Result`，`start()` 不接收 callback，输入事件由外层驱动，见 [pi-rust/crates/pi-tui/src/terminal.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/terminal.rs:72)。Rust `ProcessTerminal` 仍然有 Kitty negotiation、Windows VT input、progress thread 等复杂能力，见 [terminal.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/terminal.rs:121) 和 [terminal.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/terminal.rs:154)。

Editor 覆盖度较高。TS editor 有多行 state、padding、autocomplete、AbortController/debounce、paste tracking、history、kill ring、jump mode、sticky visual column、undo stack，见 [pi/packages/tui/src/components/editor.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/components/editor.ts:252)。Rust editor 也有 text/cursor、viewport/scroll、theme、keybindings、kill ring、undo/redo、callbacks、history、jump mode、pastes、autocomplete state/items，见 [pi-rust/crates/pi-tui/src/components/editor.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/components/editor.rs:48)。差异是 TS editor 直接持有 `TUI` 并调用 `requestRender()`，Rust editor 是纯组件，需要上层驱动重绘；TS autocomplete 是异步可取消，Rust autocomplete 是同步 trait。

Autocomplete 差距比较大。TS `CombinedAutocompleteProvider` 使用 `fd` 做全树搜索，支持 hidden、follow、exclude `.git`、AbortSignal，见 [pi/packages/tui/src/autocomplete.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/autocomplete.ts:123)。Rust provider 用 `std::fs::read_dir` 做当前 scope 同步建议，同时加入 env 变量补全，见 [pi-rust/crates/pi-tui/src/autocomplete.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/autocomplete.rs:75)、[autocomplete.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/autocomplete.rs:103)。Rust 设计更简单、确定、测试友好，但不等价于 TS 的大型项目路径补全体验。

Terminal image 方面，Rust 有 `ImageProtocol`、`TerminalCapabilities`、`CellDimensions`、`ImageRenderOptions`、capability detection、Kitty/iTerm2 编码和尺寸解析，见 [pi-rust/crates/pi-tui/src/terminal_image.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/terminal_image.rs:3)、[terminal_image.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/terminal_image.rs:77)。TS 还提供全局 `getCapabilities/setCapabilities/resetCapabilitiesCache`、`getCellDimensions/setCellDimensions`、`allocateImageId` 等便利 API，见 [pi/packages/tui/src/index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/index.ts:71)。Rust 更显式注入 capability/cell size，测试和纯函数更好，但上层使用需要更多 wiring。

**设计合理性**

整体设计合理，而且比 `pi-coding-agent` 更接近“可独立作为库使用”。

好的地方：

1. Rust 模块边界和 TS 包结构高度一致，迁移阅读成本低。
2. `Terminal` trait + `VirtualTerminal` 是很好的测试设计，避免真实 terminal I/O 参与大部分测试。
3. `Tui<T: Terminal>` 泛型化，比 TS 的运行时对象更适合 Rust 单元测试和嵌入。
4. `ComponentId` 和 `OverlayHandle` ID token 避免了对象引用生命周期问题，符合 Rust 所有权模型。
5. 样式和主题从 TS 的“函数染色”改为 `Style/Color/ThemePalette` 结构化表达，更适合 Rust 和后续 serde/配置桥接。

设计问题：

1. `Component::as_any()` 默认 `panic!` 不够稳。下游组件如果忘记实现 downcast，`component_as` 可能在运行时炸掉，见 [component.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/component.rs:20)。更稳的默认应该返回 `None` 风格，或者把 downcast 能力拆成可选 trait。
2. `pi-tui` 的默认 keybindings 已经包含 `app.model.*` 和 `app.tree.*` 这类 coding-agent 应用级 keybinding。TS `pi-tui` 的默认 `TUI_KEYBINDINGS` 基本停在 `tui.editor/input/select`，见 [pi/packages/tui/src/keybindings.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/tui/src/keybindings.ts:4)。Rust 把 app 语义放进基础 TUI crate，会让职责边界变脏。
3. TS `TUI` 内建 render scheduling，Rust 把 `RenderScheduler` 单独放出来但没有和 `Tui` 生命周期深度整合，见 [pi-rust/crates/pi-tui/src/lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/lib.rs:42)。这让上层需要自己写 event loop，灵活但容易重复。
4. Overlay focus restore 行为弱于 TS。对于多层 overlay、non-capturing overlay、临时隐藏/恢复等复杂场景，Rust 当前设计可能需要继续补齐。

**职责边界清晰度**

整体职责边界比较清楚：`pi-tui` 是底层 terminal UI 库，不应该知道 coding-agent 的 session/model/provider 语义。大部分模块都守住了这个边界：terminal、input、components、style、theme、image、markdown、fuzzy、utils 都是通用 TUI 能力。

边界不够清楚的地方主要有两个：

1. 默认 keybindings 混入了 `app.model`、`app.tree`。这属于 `pi-coding-agent` 或更上层 app，不属于通用 TUI crate。建议把 TUI 默认 keybindings 保持通用，把 app-specific definitions 由 `pi-coding-agent` 扩展注册。
2. `SelectorDialog`、`SettingsListOptions` 这类组件本身可以是通用组件，但如果字段或文案继续向 coding-agent 设置页靠拢，需要注意不要让 `pi-tui` 变成 coding-agent UI 组件库。

和 TS 相比，Rust 的职责边界反而更清晰一些：TS `TUI` 对 Node process stdin/stdout、timers、env、terminal probing 都耦合较深；Rust 把 `Terminal` trait、`RenderScheduler`、`VirtualTerminal` 拆出来，适合被不同上层 runtime 组合。

**公共接口稳定性**

稳定性中等。它已经有 public API 测试，且导出面比 `pi-coding-agent` 更像真实库 API。`tests/public_api.rs` 会导入 `Tui`、`VirtualTerminal`、components、theme、terminal image、autocomplete、style 等，能防止大面积误删。

风险也比较明显：

1. 所有模块都是 `pub mod`，内部模块天然变成公共 API，见 [pi-rust/crates/pi-tui/src/lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-tui/src/lib.rs:1)。如果只想稳定 re-export facade，应减少直接 public module 面。
2. 很多 public struct 没有 `#[non_exhaustive]`，例如 `TerminalCapabilities`、`CellDimensions`、`ImageRenderOptions`、`OverlayOptions` 等。字段新增会破坏外部结构体字面量初始化。
3. `Tui<T: Terminal>` 的泛型 API 很好用，但与 TS `TUI` 的单类 API 不同。若目标是“行为 parity”没问题；若目标是“API parity”，这已经是 Rust-native 新 API，不应宣称等价。
4. callback 都是 `Box<dyn FnMut...>`，后续如果需要 Send/Sync/async，会是 breaking change。现在要提前决定 pi-tui 是否只服务单线程 terminal loop。
5. `Component` trait 当前把 render 设为 `&mut self`，这对缓存友好，但会影响组件复用和并发渲染。不是错误，但这是稳定前需要确认的核心设计。

**建议优先级**

1. 先把职责边界清理掉：从 `pi-tui` 默认 keybindings 中移出 `app.*`，让 `pi-coding-agent` 注入应用级 bindings。
2. 给公共 API 定稳定策略：要么只承诺 `lib.rs` re-export，内部模块改 `pub(crate)` 或文档标注不稳定；要么接受所有模块 public 并加 `#[non_exhaustive]`。
3. 补齐 `Tui` runtime lifecycle：明确 Rust 上层 event loop 标准写法，`start/stop/input/resize/render scheduling` 是否由 `pi-tui` 提供一个 `run` helper，还是永远由 app 组合。
4. 改善 `Component` downcast 默认行为，避免 public trait 的默认 panic 成为下游隐患。
5. 选择 autocomplete 方向：如果要接近 TS 体验，需要异步 provider 或后台 task 接口，以及 `fd`/ignore-aware 搜索；如果保留同步接口，应明确这是 Rust 版的简化设计。
6. 为 overlay focus restore 增加 parity 测试，尤其是 nested overlay、non-capturing、hidden/show、unfocus target、visible callback 变化。
7. 保持 `VirtualTerminal` 和 integration tests 的投入。Rust 侧目前测试覆盖方向很好，适合继续用 TS regression test 名单逐项迁移。

这次我没有运行测试，只做了静态对比和结构审阅；没有修改任何文件。
