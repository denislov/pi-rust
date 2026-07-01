**当前定位更新（2026-07-02）**

这份文档保留为 `pi-agent-core` 早期对照 TypeScript `pi/packages/agent` 的历史评估。它不再是 TS parity checklist，也不再定义 `pi-rust` 的 agent runtime 推进目标。

当前项目方向以 Flow-centered runtime 为准：`CodingAgentSession` 是产品 runtime owner，`PromptTurnFlow` 负责产品 prompt turn，`AgentTurnFlow` 负责低层 agent loop。TypeScript `pi` 仍是 agent 行为、工具执行语义、compaction/branch summary 产品体验和测试 fixture 的参考，但 `pi-rust` 不追求把 Rust `AgentHarness` 做成 TS `AgentHarness` 的同构替代。

因此，本文中仍值得推进的主线是：

- 收窄 `pi-agent-core` public exports，把 `AgentTurnFlow`、legacy `agent_loop`、session JSONL 和内部 helper 与稳定公共 API 区分开；
- 强化低层 `Agent` / `AgentEvent` / tool / hook / `ExecutionEnv` contract，保持它们服务于 core runtime，而不是承担产品 session 或 UI wire protocol；
- 为工具参数验证、argument preparation、tool result details 和 plugin tool 边界建立 Rust-native contract；
- 推动 first-party tools、resource discovery 和未来 self-healing workflow 通过 `ExecutionEnv`，减少直接本地 `std::fs` 依赖；
- 保持 runtime compaction 与 Rust-native session compaction 的边界，前者属于 `AgentTurnFlow`，后者属于 `pi-coding-agent` 的产品 Flow / `SessionService`；
- 把 TS compaction、branch summary、message lifecycle 的经验作为 `CodingAgentEvent` 映射和 Phase 6 workflows 的行为参考。

本文中不再作为目标推进的内容是：TS `Agent` / `AgentHarness` 同构、TS session JSONL 兼容、让 `pi-agent-core::AgentHarness` 拥有产品 session/model/env 生命周期、将低层 `AgentEvent` 改造成产品/RPC/TUI wire protocol、以及按 TS harness feature list 机械补齐 core API。

**结论**

`pi-rust/crates/pi-agent-core` 的移植深度明显高于“占位”：agent loop、工具调用、队列、hooks、resources、session JSONL、compaction、branch summary、proxy、harness observer/on hook 等都已经有实现和测试。相比刚才的 `pi-ai`，`pi-agent-core` 更接近 TS `pi/packages/agent` 的功能轮廓。

但它仍不是 TS `@earendil-works/pi-agent-core` 的等价替代。主要差距集中在三处：TS harness 是“session/env/models/tool/resource 的统一运行时”，Rust harness 目前更像围绕 `Agent` 的生命周期适配层；TS 的事件协议更偏 UI/message lifecycle，Rust 事件更偏底层 loop；Rust session/resource/repo 多处直接绑定本地 `std::fs`，没有完全接上 TS 的 `ExecutionEnv` 抽象。

我本地跑了 `cargo test -p pi-agent-core`，全部通过：114 个单元测试和所有集成测试均通过。

**功能完整度**

| 维度 | TS `pi/packages/agent` | Rust `pi-agent-core` | 评估 |
|---|---|---|---|
| Agent 状态封装 | `Agent` 有订阅、`state`、streaming 状态、pending tool calls、abort、waitForIdle、steer/followUp，见 [agent.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/agent.ts:166) | `Agent` 是共享状态 + `AgentStream` runner，有 prompt/run/abort/queues/resources，见 [agent.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/agent.rs:39) | 核心可用，但 UI-facing 状态语义不完整 |
| Agent loop | 支持多轮、工具调用、parallel/sequential、before/after hooks、prepareNextTurn、steering/follow-up，见 [agent-loop.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/agent-loop.ts:155) | 同样支持主要 loop 行为，并加了自动 compaction 前置，见 [agent_loop.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/agent_loop.rs:197) | 功能覆盖较高 |
| 工具执行 | TS 有 schema validation、`prepareArguments`、update callback、after patch `details/isError/terminate`，见 [agent-loop.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/agent-loop.ts:562) | Rust 有 parallel/sequential、before/after、updates、terminate；但 schema validation/prepareArguments 语义弱，`details` 在 tool result message 中丢失，见 [types.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/types.rs:368) | 主流程可用，类型/细节不全 |
| 事件协议 | TS `AgentEvent` 是 `agent_start/message_start/message_update/message_end/turn_end/agent_end/tool_execution_*`，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/types.ts:413) | Rust `AgentEvent` 是 `TurnStart/BeforeProviderRequest/LlmEvent/ToolCall*/AgentDone/AgentError/SessionCompacted`，见 [types.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/types.rs:457) | 不等价，适配 UI 需额外层 |
| Harness | TS harness 持有 env/session/models/tools/resources，负责 session writes、compaction/tree、model/tool/resource updates，见 [agent-harness.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/agent-harness.ts:157) | Rust harness 有 hooks、phase、subscribe/on、provider request/payload/response hook、auth patch，见 [harness.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/harness.rs:227) | 生命周期 hook 做得不错，但还不是完整 TS harness |
| Session | TS 有强类型 `SessionStorage`/`Session`/entry union，append/moveTo/buildContext，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/types.ts:440)、[session.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/session/session.ts:82) | Rust 有 JSONL storage/repo/tree/leaf/label/migration/context，见 [jsonl.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/session/jsonl.rs:12)、[context.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/session/context.rs:245) | 功能很多，但 public entry 类型偏动态 |
| ExecutionEnv | TS `FileSystem`/`Shell` 是 harness 核心抽象，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/types.ts:252) | Rust 有 trait 和 `InMemoryExecutionEnv`，见 [env.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/env.rs:35) | 抽象存在，但 resources/session repo 未全面使用 |
| Resources | TS skills/templates 基于 `ExecutionEnv`，支持 diagnostics、ignore、source tagging，见 [skills.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/skills.ts:44) | Rust 支持 skills/templates、ignore、frontmatter、source tagging，见 [skills.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/resources/skills.rs:10) | 功能接近，但绑定本地 FS |
| Compaction | TS 是 session-tree 级：cut point、details、hook、persist entry，见 [compaction.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/compaction/compaction.ts:78) | Rust 有 estimate/prepare/summarize，并在 loop 前自动压缩消息，见 [prepare.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/compaction/prepare.rs:7) | 可用但层级不同，session 持久化未完整对齐 |
| Proxy/Branch summary | TS 有 proxy 和 branch summarization | Rust 已有 proxy 和 branch summary，见 [proxy.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/proxy.rs:1)、[branch_summary.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/branch_summary.rs:1) | 覆盖较好 |

**关键差距**

1. **Rust `Agent` 不是 TS `Agent` 的同构实现。**
   TS `Agent` 是状态化 UI runtime：`state.isStreaming`、`streamingMessage`、`pendingToolCalls`、`errorMessage`、listener settlement 都是公共契约，见 [agent.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/agent.ts:509)。Rust `Agent` 返回 stream，内部用 `RwLock<AgentState>` 和 `AtomicBool` 管并发，见 [agent.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/agent.rs:14)。它更适合 CLI/服务端消费，不直接满足 TS UI 状态模型。

2. **事件协议不稳定且不对齐。**
   TS loop 事件围绕消息生命周期，任何 UI 可以用 `message_start/update/end` 增量更新。Rust 直接暴露 `LlmEvent(AssistantMessageEvent)` 和 `ToolCall*`，需要上层自己还原 message lifecycle。这个差异会影响 `pi-tui` 或未来 web UI 的复用成本。

3. **Harness 缺少 TS 的 session-owner 职责。**
   TS harness 在一次 turn 中会创建 turn state、从 session 构建上下文、写 pending session mutations、在 `turn_end` 做 save point、支持 `compact()` 和 `navigateTree()`，见 [agent-harness.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/agent/src/harness/agent-harness.ts:488)。Rust harness 当前只封装 `Agent`，能 emit/observe/hook provider request，但没有真正持有 `Session`、`Models`、`ExecutionEnv`，也没有完整的 pending session write 机制。

4. **Session 类型表达比 TS 弱。**
   TS `SessionTreeEntry` 是严格 discriminated union；Rust `SessionEntry` 是 `entry_type + fields: Map<String, Value>`，见 [types.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/session/types.rs:19)。这对迁移旧 JSONL 和保留未知字段友好，但 public API 的类型安全、IDE 可发现性、编译期约束都弱于 TS。

5. **`ExecutionEnv` 抽象没有贯穿。**
   Rust 有 `FileSystem/Shell/ExecutionEnv` trait 和内存实现，但 `JsonlSessionRepo`、resources loader 仍主要走 `std::fs`/`PathBuf`。TS 的 repo/resources 都走 `FileSystem`，所以更容易在远端、沙箱、测试环境复用。Rust 后续如果要支持非本地环境，需要重构边界。

6. **工具 schema/参数验证没有 TS 完整。**
   TS 工具继承 `Tool<TSchema>`，通过 `validateToolArguments` 做 schema 校验，并支持 `prepareArguments`。Rust `AgentTool.parameters` 是 `serde_json::Value`，执行函数直接拿 JSON。Rust 可以运行，但少了 TS 的参数准备和验证 contract。

7. **Compaction 层级偏离。**
   Rust loop 会在 provider request 前估算并直接压缩 `AgentState.messages`，见 [agent_loop.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/agent_loop.rs:73)。TS harness compaction 是 session tree 操作，会产生 compaction entry、details、hook 结果和可导航历史。这意味着 Rust 自动 compaction 当前更像“运行时消息裁剪/总结”，不是完整“持久会话压缩”。

**设计合理性**

好的部分：

- Rust 已经把大块逻辑拆成 `agent`、`agent_loop`、`loop_runtime`、`hooks`、`harness`、`session`、`resources`、`compaction`，比 TS 单文件大类更便于局部测试。
- `loop_runtime::tools` 和 `loop_runtime::context` 抽出来是正确方向，避免 agent loop 无限膨胀。
- `AgentHooks` 和 `AgentHarnessHooks` 分层合理：低层 loop hook 与高层 harness provider hook 分开。
- JSONL session 支持 v1/v2 migration、leaf、label、tree、fork，说明迁移兼容性考虑充分。
- `SubscriptionGuard` 用 RAII 管订阅生命周期，比手动 unsubscribe 更符合 Rust。
- 测试覆盖扎实，尤其 parallel tools、queues/thinking、session JSONL/tree、harness hooks、proxy/branch summary 都有独立测试。

问题部分：

- `lib.rs` 直接 `pub mod` 暴露了几乎所有模块，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/lib.rs:1)。这会把内部结构变成事实公共 API。
- `AgentState` 是 public struct 且字段 public，见 [agent.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/agent.rs:14)。这对未来调整锁粒度、状态机、事件模型很不利。
- `AgentHarnessPhase` 注释承认 `Compaction` 和 `BranchSummary` 目前只是预留，见 [harness.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/harness.rs:198)。公共枚举已经暴露了未实现状态，稳定性风险较高。
- loop 内部仍很长，尤其工具执行 sequential/parallel 两套路径在 [agent_loop.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-agent-core/src/agent_loop.rs:440) 之后展开，后续维护成本会升高。
- 同时存在 `hooks.rs` 的 loop hooks 和 `harness.rs` 的 harness hooks，命名相近但语义不同，调用者容易混淆。

**职责边界清晰度**

TS 的职责边界更偏“产品运行时”：

- `Agent`：内存态 agent + UI event reducer。
- `agent-loop`：纯 loop。
- `AgentHarness`：session/env/models/tools/resources 的持久运行时。
- `Session`/`Repo`/`Storage`：session tree。
- `ExecutionEnv`：文件系统和 shell 能力边界。
- resources/compaction/proxy：可复用工具。

Rust 的职责边界当前是“核心 runtime + 若干移植模块”：

- `Agent` 和 `agent_loop` 边界清楚，但 `Agent` 还暴露过多状态。
- `harness` 的目标方向对，但还没完整拥有 TS harness 的 session/env/models 生命周期。
- `session` 独立性不错，但没有通过 trait 抽象 storage/repo；目前更像 concrete JSONL API。
- `resources` 功能独立，但未使用 `ExecutionEnv`，和 `env` 模块边界没有接起来。
- `compaction` 算法独立性不错，但和 `Agent` 自动压缩、session 持久压缩之间的职责还没完全分清。

**公共接口稳定性**

我会把当前 Rust `pi-agent-core` 公共接口稳定性评为 **中等偏低**。

原因不是实现质量差，而是暴露面过宽、部分 API 还不是 TS 等价形态：

- crate 仍是 `0.1.0`。
- `lib.rs` 暴露了 `agent_loop`、`convert`、`proxy`、`queues`、`resources`、`session` 等完整模块。
- 许多 public struct 字段裸露：`AgentConfig`、`AgentTool`、`AgentEvent`、`SessionEntry`、`JsonlSessionStorage`。
- TS 等价 contract 尚未定型：尤其 `AgentEvent`、`AgentHarness`、session entry 类型、ExecutionEnv 接入方式。
- 一些 public 类型是未来预留或半成品状态，如 harness phase 的 `Compaction/BranchSummary`。

**建议优先级（按当前 Flow-centered 方向重述）**

1. **明确 `pi-agent-core` 是 Rust-native low-level runtime，而不是 TS harness 同构层。**
   `AgentTurnFlow`、`Agent`、tools、hooks、queues、runtime compaction、`ExecutionEnv` 和 low-level `AgentEvent` 属于 core。产品 session ownership、Rust-native session log、adapter state、RPC/TUI event wire 和 product workflow ownership 属于 `pi-coding-agent::CodingAgentSession` / `CodingAgentEvent`。

2. **收窄 public exports 和内部状态暴露。**
   保留稳定入口：`Agent`、`AgentConfig`、`AgentTool`、`AgentEvent`、必要 hooks、`ExecutionEnv` 和少量可复用 helper。`AgentTurnFlow` 具体节点、legacy `agent_loop` wrapper、JSONL session compatibility modules、资源加载 internals 和 `AgentState` 等应逐步标记为 migration-private、unstable 或收窄为内部实现细节。

3. **稳定低层事件，并把产品生命周期映射留在 `CodingAgentEvent`。**
   不建议把 `AgentEvent` 改造成 TS-style UI event protocol。更合适的方向是保持 `AgentEvent` 低层、完整、可映射，并用 `pi-coding-agent::EventService` 把它转换成 message/tool lifecycle 友好的 `CodingAgentEvent`。TS 的 `message_start/update/end` 经验应主要用于 product event mapping tests。

4. **不要让 `AgentHarness` 重新拥有产品 session；保留它作为 core lifecycle/hook facade。**
   TS harness 的 session/env/models/tools/resources 统一 owner 职责已经由 Rust 的 `CodingAgentSession`、`RuntimeService`、`SessionService` 和 plugin services 拆分承接。Rust `AgentHarness` 若继续存在，应聚焦 low-level observer/hook/provider request 边界，而不是恢复为产品 runtime owner。

5. **把 first-party tools、resources 和未来 workflows 接到 `ExecutionEnv`。**
   `ExecutionEnv` 已经是 core 的文件系统和 shell 能力边界。后续应优先让 first-party tools、resource discovery、self-healing edit workflow 等通过 trait 访问环境。旧 JSONL session repo 是否接入 `ExecutionEnv` 只作为 legacy 维护问题，不应反向影响 Rust-native session log。

6. **补工具参数验证和 argument preparation 的 Rust-native contract。**
   Rust 至少应提供 schema validation hook、argument normalization/preparation hook，或明确声明 core 不做执行期校验。验证失败应产生一致的 tool error result，并与 plugin tool 注册边界复用同一套规则。

7. **继续区分 runtime compaction 和 product session compaction。**
   `AgentTurnFlow` 的 compaction 只影响 provider context 和低层 `AgentEvent::SessionCompacted`，不写产品 session。持久 compaction、branch summary、tree navigation 和 new leaf 创建应通过 `pi-coding-agent` 的 Phase 6 workflows / `SessionService` 实现。

**总体判断**

`pi-agent-core` 的 Rust 移植已经达到“可跑核心 agent loop，并可支撑相当多 PoC/CLI 场景”的程度。它比 `pi-ai` 更接近 TS 参考实现，尤其 session、resources、proxy、branch summary 和 harness hooks 都有实质实现与测试。

但从“完整替代 TS `packages/agent`”角度看，还缺一个关键整合层：TS 的 `AgentHarness` 是整个 agent core 的产品边界，而 Rust 目前仍是多个能力模块并列存在。下一阶段最该做的是把 `AgentHarness + Session + ExecutionEnv + Models/streamer` 的职责闭合起来，同时收窄公共 API，避免当前半稳定结构被上层 crate 固化。
