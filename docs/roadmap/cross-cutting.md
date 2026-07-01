# 横切项：辅助 crate · 架构约束 · 风险

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md)
> 不属于单一 Phase、但贯穿全程的事项。

## 1. 辅助 crate 定位（pi-mom / pi-pods / pi-web-ui）

- 三者均为 14 行 `cargo new` 桩，**不在**当前 Flow-centered runtime 主线内，无明确 TS 来源或产品设计。
- **行动项**：投入前先明确各自目标范围（对应 TS 侧能力，或新的 Rust-native 产品需求）；否则保持空壳。
- 当前默认：暂缓。等 `pi-coding-agent` 插件/runtime 边界稳定后再评估是否保留、合并或移除。

## 2. 当前兼容策略

- **会话**：不再保持 TypeScript `pi` session JSONL 互通。当前持久化格式是 Rust-native `session.json` + typed `events.jsonl`，见 [Rust-native session format](../superpowers/specs/2026-07-01-rust-native-session-format.md)。旧 TS JSONL/旧 Rust runner 路径只作为迁移背景或显式拒绝对象。
- **配置 / 认证**：使用 Rust-native 格式，不要求读取 TS pi 的 `settings.json` / `auth.json`。旧 `PI_AGENT_DIR` 不是默认 session 根目录。
- **Provider wire JSON**：provider/API 层仍应尽量保持与目标上游协议字节级一致，由 serde 转换和 fixture 测试守护。
- **Adapter protocol**：JSON/RPC/TUI wire 可保持现有客户端兼容，但不得反向约束 session log 或 Flow 内部节点形状。

## 3. 架构约束

- **Flow 边界清晰**：`PromptTurnFlow` 负责产品 turn；`AgentTurnFlow` 负责低层 agent loop；Phase 6 的 first-party workflows 不应把两者重新揉成大对象。
- **`pi-ai` 只拥有 provider/model/auth runtime**：`pi-ai` 可以发展 scoped `AiClient`、provider registry、auth resolver、transport hooks 和 catalog invariants，但不得依赖 `CodingAgentSession`、Rust-native session log、CLI/RPC/TUI 或产品 Flow。
- **`pi-agent-core` 只拥有低层 agent runtime 能力**：core 可暴露 `Agent`、`AgentTurnFlow`、tool/hook/queue/runtime compaction、`ExecutionEnv` 和低层 `AgentEvent`，但不写 `CodingAgentEvent`、session manifest、Rust-native session log、adapter state 或产品 workflow ownership。
- **`CodingAgentSession` 是产品 owner/coordinator**：共享产品能力应进入 session-owned services/flows，再由 print/RPC/interactive adapters 消费；稳定公共面优先通过 `pi_coding_agent::api`，根模块和 adapter internals 不应成为长期 SDK 承诺；owner 可以编排服务和事务，但不应重新变成包含所有实现细节的 monolithic class。
- **`pi-tui` 是通用 terminal UI crate**：`pi-tui` 拥有 terminal/input/component/render/overlay/theme/image/autocomplete 等底层 UI primitive 和 `VirtualTerminal` 测试能力，但不拥有 coding-agent 的 model/session/tree/tool/plugin 产品语义；`app.*` keybindings、interactive product actions 和 plugin UI/keybind dispatch 应由 `pi-coding-agent` 的受控 adapter 注入。
- **插件 API 不依赖内部 operation context**：Phase 5 的 `ToolProvider` / `CommandProvider` / `HookProvider` / UI 边界应只暴露稳定 capability context。
- **Product event adapters 不依赖具体 Flow node id**：adapter 只消费 `CodingAgentEvent` 语义，不靠节点名推断状态。
- **测试默认离线确定**：真实 provider、终端 smoke、网络或系统集成验证必须显式 opt-in。
- **工作树纪律**：不回退/覆盖他人改动；`pi/` 与 `pi-rust/` 是两个独立 git 仓库，分别操作。

## 4. 风险

| 风险 | 说明 | 缓解 |
|---|---|---|
| Rust-native session schema 漂移 | `events.jsonl` 是当前产品会话事实来源，事件 kind/字段变更会影响 replay、resume、fork/clone/tree/compact | 保持 schema/version；新增字段优先后向兼容；用 replay fixture 和 focused tests 守护 |
| 旧 TS parity 文档误导 | 早期 M7-M13 和对标报告包含“迁移 TS”等历史说法，可能与 Flow-centered 决策冲突 | `ROADMAP.md` 和 `docs/TODO.md` 作为当前索引；旧文档标记为背景而非约束 |
| 插件边界过早泄漏内部类型 | Phase 5 若直接暴露 `PromptTurnContext` / operation internals，会固化实现细节 | 先定义 capability-scoped provider traits 和 failure isolation，再接 RuntimeService/PromptTurnFlow |
| Flow 与 owner 责任重叠 | 新 workflow 容易重复 session commit、active leaf、event emit 逻辑 | `SessionService` 继续拥有持久化 finalization；Flow 节点只通过明确服务边界操作 |
| `pi-ai` runtime/auth 边界漂移 | 若全局 registry、provider auth、config/auth store、product runtime 各自扩张，会形成难以测试的多实例和插件边界问题 | 规划 Rust-native scoped `AiClient` / auth resolver；把 global `register()` / `stream_model()` 降为兼容或测试辅助；禁止 `pi-ai` 依赖 coding-agent 产品语义 |
| `pi-agent-core` TS harness parity 误导 | 旧对标报告容易把 TS `AgentHarness` 的 product owner 职责重新推回 core，导致 `CodingAgentSession`、`SessionService`、`RuntimeService` 和 `AgentTurnFlow` 边界重叠 | 将 TS harness 行为拆成参考：低层 loop/tool/hook/env 留在 core；session persistence、product events、manual compaction、branch summary 和 tree workflow 留在 `pi-coding-agent` |
| `pi-coding-agent` TS SDK parity 误导 | 旧对标报告容易把 TS root SDK、extension runtime、unknown CLI flags 或旧 JSONL runner parity 当成目标，导致 public API 过宽、adapter 各自加业务逻辑、plugin 权限绕过 scoped host | 只承诺 `pi_coding_agent::api`；共享操作进入 `CodingAgentSession` services/flows；adapter 功能通过 capabilities/protocol versions 暴露；plugin/extension 能力走 scoped hosts、permission/trust 和 `PluginLoadFlow` |
| `pi-tui` 产品语义泄漏 | 旧 TUI 对标报告容易把 TS package parity、`TUI` lifecycle shape 或 coding-agent app keybindings 当成 base crate 目标，导致通用 UI crate 固化产品语义、插件 UI 绕过 scoped adapter、公共 API 过早变宽 | `pi-tui` 保持 generic；`app.*` keybindings 和 product actions 放回 `pi-coding-agent`；plugin UI/keybind 只通过 capability-scoped adapter 接入；稳定 public facade 后再扩展模块承诺 |
| 模型表漂移 | 生成式注册表已落地，但上游模型更新需重新生成 | 保留生成脚本，定期重生成并用 fixture 测试 |
| 主题/终端差异 | 256 色、宽度、按键协议、图片协议在终端间差异大 | 继续维护 TUI smoke 套件，保持 opt-in |
| 认证子系统体量 | OAuth/SigV4/WebSocket 各自独立 | 逐 provider spec -> plan -> 实现，避免一次性重写 |
| 辅助 crate 无方向 | `pi-mom` / `pi-pods` / `pi-web-ui` 范围未定，贸然投入有返工风险 | 先定范围再投入；无产品需求时保持空壳 |
