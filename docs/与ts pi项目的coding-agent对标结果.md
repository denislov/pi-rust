**当前定位更新（2026-07-02）**

这份文档保留为 `pi-coding-agent` 早期对照 TypeScript `pi/packages/coding-agent` 的历史评估。它不再是 TS parity checklist，也不再定义 `pi-rust` 的产品层推进目标。

当前项目方向以 Flow-centered runtime 为准：`CodingAgentSession` 是产品 runtime owner，`PromptTurnFlow` 是 prompt turn 主路径，`CodingAgentEvent` 是 CLI/RPC/interactive 适配器的 canonical product event stream，Rust-native `session.json` + typed `events.jsonl` 是当前会话事实来源。TypeScript `pi` 仍是产品行为、UX、命令面、RPC 能力、插件体验和测试 fixture 的参考，但 `pi-rust` 不追求 TS `coding-agent` SDK/root export/extension runtime 的同构替代。

因此，本文中仍值得推进的主线是：

- 收窄 `pi_coding_agent::api` 稳定入口，把根模块、adapter internals、protocol internals 和迁移期 helper 与长期公共 API 区分开；
- 继续让 `CodingAgentSession` / `SessionService` / `RuntimeService` / `FlowService` 承接 TS `AgentSession` 的产品 owner 行为，而不是恢复旧 `session_runner` 或各 adapter 私有状态机；
- 以 `CodingAgentCapabilities` / `CapabilityStatus` 和协议版本表达 RPC/TUI/JSON 可用能力，避免 adapter 命令面先声明、handler 再返回临时 unsupported 字符串；
- 推进 plugin command/UI/keybind execution、`PluginLoadFlow`、trust/permission gating 和 scoped host API，借鉴 TS extension 体验但不暴露 raw session/auth/provider internals；
- 把 manual compaction、branch summary、export、session switch、subagent/supervisor、自修复 edit 等产品能力落到 Phase 6 product Flows，而不是旧 JSONL runner 或低层 core；
- 保持配置/认证/session 格式 Rust-native，仅把 TS settings/commands/UX 字段作为产品需求参考。

本文中不再作为目标推进的内容是：TS `coding-agent` 完整 port、TS session JSONL 兼容、TS settings/auth 文件兼容、TS root SDK export parity、unknown CLI flags 直接透传给 extension runtime、以及围绕旧 `session_runner` / M5 RPC 子集继续扩展。

**总体结论**

按 TypeScript `pi/packages/coding-agent` 作为目标完整产品来衡量，`pi-rust/crates/pi-coding-agent` 目前不是完整 port，而是一个已经能跑的 Rust coding-agent 切片：覆盖了 CLI 参数解析、print/json/rpc 基础模式、session JSONL 持久化、内置工具、资源加载、主题桥接、基础交互 TUI 和一部分 RPC/interactive workflow。它的方向是对的，但离 TS coding-agent 的“产品层 + SDK 层 + 扩展平台”还有明显距离。

我的粗略完整度判断：

| 维度 | Rust 当前完整度 | 判断 |
|---|---:|---|
| Headless/print 基础执行 | 60%-70% | 已有可用结构，支持 prompt、工具、session、资源、配置，但 provider/auth/model fallback/错误体验仍比 TS 简化 |
| 内置工具 | 70%-80% | `read/write/edit/bash/grep/find/ls` 都在，工具过滤也有；但 TS 的工具定义、操作注入、扩展包装和渲染面更完整 |
| Session/compaction | 45%-55% | JSONL open/continue/fork/name/compact 有基础；缺 branch summary、完整 stats、session switching/tree/export parity |
| Interactive TUI | 35%-45% | 有 TUI loop、slash、model/session/settings 一些能力；但 TS 的 extension UI、trust、first-time setup、完整 slash 行为未齐 |
| RPC | 35%-45% | Rust 明确是 M5 子集；TS RPC command 面大很多 |
| Extensions/package manager/project trust | 5%-15% | 基本未 port 到 Rust coding-agent 层 |
| SDK/public API parity | 25%-35% | Rust 有 `api` facade，但远小于 TS 的 SDK/extension/component export 面 |

**功能完整度**

TS coding-agent 的公共入口非常大，`src/index.ts` 直接暴露 `AgentSession`、auth/model registry、compaction、extension runtime、package manager、resource loader、SDK factory、session manager、settings manager、skills、tools、trust manager、run modes、interactive components、theme utils、clipboard/image/shell utils 等，见 [pi/packages/coding-agent/src/index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/index.ts:15)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/index.ts:61)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/index.ts:178)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/index.ts:250)、[index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/index.ts:310)。

Rust `pi-coding-agent` 已经有一个清晰的基础面：`args/config/input/interactive/protocol/request/resources/runtime/session/theme/tools` 都是 public module，并且有一个 `api` facade，见 [pi-rust/crates/pi-coding-agent/src/lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:1) 和 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:28)。它支持 `run_cli`、`run_cli_with_options`、print mode、session prompt runner、diagnostic rendering、tool filtering 等，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:120)。

CLI 参数方面，Rust 覆盖了 provider/model/models/list-models/api-key/system-prompt/thinking/tool-execution/session/skills/prompt-templates/theme/tools/offline 等，见 [args.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/args.rs:25) 和 [args.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/args.rs:121)。但 TS 还多了 extension、no-extension、export、project trust、file args、unknown extension flags 等，见 [pi/packages/coding-agent/src/cli/args.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/cli/args.ts:12)、[args.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/cli/args.ts:147)、[args.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/cli/args.ts:180)。一个关键差异是 TS 会把未知 `--flag` 收进 `unknownFlags` 留给 extensions，Rust 直接 `UnknownFlag`，见 [args.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/cli/args.ts:188) 对比 [args.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/args.rs:267)。这说明 Rust 目前没有 extension-first 的 CLI 扩展模型。

Session 方面，TS 的 `AgentSession` 是跨 print/rpc/interactive 的核心对象，明确承担状态访问、事件订阅、持久化、模型/thinking 管理、compaction、bash、session switching/branching，见 [agent-session.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/core/agent-session.ts:1)。Rust 对应的是更窄的 `SessionPromptOptions` + `run_session_prompt`/`spawn_session_prompt`，支持 abort/steer/follow_up、session path/leaf 返回、manual compaction 和 JSONL append，见 [session_runner.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/protocol/session_runner.rs:19)、[session_runner.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/protocol/session_runner.rs:44)、[session_runner.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/protocol/session_runner.rs:108)。这个切片有价值，但不是 TS `AgentSession` parity。

工具方面，Rust 已经有 7 个内置工具并统一从 `builtin_tools(cwd)` 组装，见 [tools/mod.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/tools/mod.rs:5) 和 [tools/mod.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/tools/mod.rs:17)。TS 不仅有工具，还暴露 tool definition factory、operation injection、truncate helpers、file mutation queue 等完整 SDK 面，见 [pi/packages/coding-agent/src/index.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/index.ts:250)。Rust 的工具实现更接近“agent 内置工具”，TS 是“内置工具 + 扩展平台工具定义 + 可嵌入 SDK”。

RPC 方面差距明确。TS RPC command 包括 model 切换、cycle thinking、retry、bash、export_html、switch_session、fork、clone、get_commands、extension UI 等，见 [rpc-types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/modes/rpc/rpc-types.ts:19)。Rust `RpcCommand` 目前只到 prompt/queue/basic state/compact flag/stats/messages/name 等，见 [protocol/types.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/protocol/types.rs:105)。而且 `wire.rs` 明确叫 `is_supported_m5_command`，支持列表也只是子集，见 [wire.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/protocol/rpc/wire.rs:46)。这说明当前 RPC 设计目标还停在里程碑兼容，不是最终协议完整实现。

Interactive 方面，Rust 有 slash handling，包含 `/model`、`/resume`、`/export`、`/import`、`/copy`、`/new`、`/clone`、`/settings`、`/name`、`/session`、`/login`、`/logout`、`/fork`、`/compact`、`/tree` 等，见 [commands.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/interactive/commands.rs:81)。但 `/scoped-models` 和 `/share` 只是 recognized but not implemented，见 [commands.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/interactive/commands.rs:108)。TS 内置 slash 还有 `/trust`，并且 reload 包含 extensions/skills/prompts/themes，见 [pi/packages/coding-agent/src/core/slash-commands.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/coding-agent/src/core/slash-commands.ts:17)。Rust TUI 是可用 PoC/迁移切片，不是完整产品替代。

资源/配置方面，Rust 已经做了比较实在的迁移：支持 AGENTS.md/CLAUDE.md、skills、prompt templates、themes、theme token 到 `pi_tui::TuiTheme` 的有损桥接，见 [resources.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/resources.rs:47)、[resources.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/resources.rs:104)、[resources.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/resources.rs:163)。配置字段也覆盖了 default model/provider、transport、queues、session_dir、skills/prompts/themes、terminal、compaction、retry 等，见 [settings.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/config/settings.rs:58)。但 TS settings 还包括 packages/extensions/project trust/analytics/branch summary/thinking budgets/editor/autocomplete/markdown/images 等更多产品级字段。

**设计合理性**

整体设计方向是合理的。Rust 把 agent loop、session format、resources、types 放在 `pi-agent-core`，coding-agent crate 负责 CLI/product harness，这符合迁移边界。`pi-coding-agent` 内部再拆成 `request` 解析上下文、`runtime` 构造 agent config、`session_runner` 驱动 prompt、`protocol` 适配 json/rpc、`interactive` 承接 TUI，层次基本清楚。

比较好的点：

1. `api` facade 的存在是正确方向。它给嵌入方一个“应优先使用”的稳定入口，同时保留根模块 public 方便迁移期开发，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:28)。
2. 工具系统先实现 Rust 原生内置工具，再用 `ToolFilter` 做 allow/deny/no-tools/no-builtins，设计简单可测，见 [tools/mod.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/tools/mod.rs:29)。
3. session runner 用 `spawn_session_prompt` 返回事件流和 done channel，这对 RPC/interactive 都适合，见 [session_runner.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/protocol/session_runner.rs:116)。
4. 主题迁移明确承认 51-token TS theme 到 10-color TUI palette 是 lossy bridge，这种注释很诚实，也有利于后续扩展，见 [resources.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/resources.rs:47)。

主要设计问题：

1. Rust 还缺一个等价 TS `AgentSession` 的长期核心抽象。现在 `SessionPromptOptions` 很宽，`interactive` 和 `rpc` 都在围绕它拼上下文，后续如果继续加 model cycling、extension event、branch tree、retry、bash side-channel，容易把状态散落在 `interactive::root`、`rpc::state`、`session_runner` 三处。
2. RPC 类型里已经声明了 `compact`，但 command handler 又返回“manual compaction is not available in Rust M5”。这种“类型支持但行为不支持”的状态可以接受于迁移期，但长期会损害协议稳定性。最好要么分版本协议，要么把 unavailable 标为 capability。
3. `run_cli_with_options_and_stdin` 中 RPC 被拒绝，binary main 又特殊处理 RPC streaming entry point，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:166)。这对 CLI 可行，但对库使用者不够直观。
4. 配置里用了 `deny_unknown_fields`，对 Rust 自身配置质量有利，但如果未来要兼容 TS settings 或共享 `.pi/settings.json`，会比 TS 的宽松演进更脆弱。

**职责边界清晰度**

职责边界中等偏好，但还没完全稳定。

清晰的部分：

1. `pi-agent-core` 承担 Agent、AgentMessage、AgentTool、AgentResources、session、compaction、hooks/harness 等核心抽象；`pi-coding-agent` 使用这些类型而不是重新定义 agent loop，这是正确边界。
2. `pi-coding-agent::tools` 是产品内置工具层，不把工具放进 core，也合理。
3. `resources.rs` 负责把 coding-agent 语义资源加载为 `AgentResources`，也符合“产品层资源发现，core 层执行消费”的分工。

模糊的部分：

1. TS 的 `coding-agent` 同时承担 SDK、CLI、extensions、TUI product。Rust 当前把一部分 SDK 稳定入口放在 `api`，但根模块全部 public，实际边界仍模糊：下游可以直接依赖 `interactive`、`request`、`resources`、`theme` 等迁移中模块，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:1)。
2. `interactive` 内部承担了不少 session 操作，例如 hydrate、clone、export、import、stats，而 session runner 也承担持久化。这些行为未来如果 RPC 也要完整支持，最好上移到一个 shared `AgentSession`/`SessionController` 层。
3. extension/package manager/project trust 在 TS 是 coding-agent 的产品能力，Rust 目前基本缺位。职责边界不是“清晰地不做”，而是“还没迁移到这里”。后续需要明确：Rust 是否要复刻 TS extension API，还是另设 Rust-native extension/hook 系统。

**公共接口稳定性**

当前稳定性偏低到中等，原因不是代码差，而是迁移阶段暴露面没有收口。

正面信号：

1. `api` facade 明确写着“downstream crates should prefer this module for APIs that are intended to stay stable”，这是很好的稳定性策略，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/lib.rs:28)。
2. 有 `tests/public_api.rs` 验证 facade 符号可导入，至少防止意外删掉常用入口。
3. `CliOutput`、`CliRunOptions`、`SessionPromptOptions`、`ToolFilter` 等是嵌入使用者可理解的 Rust API。

风险点：

1. 根模块全 public 会让迁移内部实现变成事实公共 API。比如 `interactive`、`protocol`、`request`、`theme` 都 public，后续重构会有破坏下游的压力。
2. `SessionPromptOptions` 字段很多且全 public，没有 builder，也没有 non-exhaustive 策略；新增字段会破坏结构体字面量初始化的下游代码。
3. `CliArgs` 是手写 public struct，字段全面暴露，后续 CLI 行为变化容易成为 API break，见 [args.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-coding-agent/src/args.rs:25)。
4. Rust public API 和 TS public API 不是同构关系。TS 导出的是面向 npm/extension ecosystem 的大 SDK；Rust 当前导出的是嵌入 CLI/session runner 的小 SDK。若目标是 TS SDK parity，公共接口还远未稳定；若目标是 Rust-native CLI harness，则需要明确“只稳定 `api`，根模块迁移期不承诺”。

**建议优先级（按当前 Flow-centered 方向重述）**

1. **明确 `pi-coding-agent` 是 Rust-native 产品 owner，而不是 TS coding-agent 同构层。**
   当前稳定方向是 `CodingAgentSession` + internal services + product Flows + `CodingAgentEvent`。TypeScript `AgentSession` 的产品行为可作为参考，但不应引入另一个同构 owner，也不应回到旧 `session_runner` / JSONL product path。

2. **收口 public API 到 `pi_coding_agent::api`。**
   根模块仍公开的 `interactive`、`protocol`、`request`、`resources`、`theme`、`runtime` 等应视为迁移期内部面。长期公共入口应优先通过 `api` facade 暴露；对 options 类型考虑 builder、constructor 或 `#[non_exhaustive]`，避免下游用结构体字面量固化迁移期字段。

3. **继续把 shared product operations 上移到 `CodingAgentSession` services/flows。**
   session stats、export、clone/fork/tree、manual compaction、model/thinking switching、session switch、branch summary、subagent/supervisor 和 self-healing edit 都应通过 shared service/Flow 暴露，再由 print/RPC/interactive 适配器复用。adapter 不应各自维护平行业务状态机。

4. **把 RPC/TUI/JSON 能力表达成 capability/version contract。**
   已有 `CodingAgentCapabilities` / `CapabilityStatus` 是正确方向。后续新增 TS 参考中的 retry、bash side-channel、export、switch session、plugin commands、extension UI 等能力时，应先进入 capability model 和协议版本规划，再接 handler。

5. **推进 Rust-native plugin/extension 产品链路，而不是照搬 TS extension runtime。**
   Phase 5 已有内部 Rust trait kernel。下一步重点是 command execution、UI/keybind dispatch、`PluginLoadFlow`、trust/permission gating、plugin diagnostics 和 adapter capability exposure。TS unknown CLI flags、package manager、extension UI 可作为体验参考，但不能绕过 scoped host 和 permission 边界。

6. **保持 Rust-native config/auth/session 格式，同时借鉴 TS 产品字段。**
   不读取 TS `.pi` settings/auth/session 作为默认兼容目标。可以把 TS 的 project trust、plugin/package source、branch summary policy、thinking budget、editor/autocomplete、markdown/images、analytics 等作为 Rust-native 设置需求候选，并通过严格 schema、migration 和 capability exposure 管理。

我没有运行测试；这次只做了静态对比和结构审阅。当前工作区里 `pi-rust` 有未跟踪文档文件，我没有修改任何文件。
