# `pi-rust` Breaking Architecture Convergence 执行计划

> 目标版本：Rust crates `0.2.0`，live protocol family major `2`
>
> 基线提交：`ae367e2 chore: checkpoint crate sources`
>
> 状态：执行中（M0 已完成，M1 进行中）
>
> 依据：`docs/architecture.md`、`docs/code-cleanup-strategies.md` 和 2026-07-15 工作区源码

## 执行快照（2026-07-15）

| Milestone | 状态 | 已落地内容 | 下一退出条件 |
|---|---|---|---|
| M0 | 完成 | contract inventory、session compatibility fixtures、dead-code inventory、architecture gates | 持续保持 gates 通过 |
| M1 | 进行中 | core 强制 injected provider streamer；旧 monolithic agent loop 已删除；scoped registry 并行隔离测试已建立 | product runtime 删除 global test bridge；lower-level facade 收窄 |
| M2-M7 | 未开始 | - | 按依赖顺序执行 |

已提交检查点：

- `e561134`：建立 architecture convergence baseline。
- `0544b97`：删除 retired agent loop。
- `8e36a8a`：`pi-agent-core` 强制 scoped provider streaming。
- `465f236`：修正 product Flow 测试的显式 provider 注入。
- `7354cae`：证明两个同名 API 的并行 scoped registry 无串扰。

M1 当前剩余风险是 `pi-coding-agent::RuntimeService` 在 test/debug 构建下仍可从 deprecated global registry 填充临时 `AiClient`。在 CLI、RPC、interactive 和 session owner 完成显式 `AiClient` 传递、测试夹具迁移完成之前，不得删除该桥接，也不得将 WP1.1 标记完成。

## 1. 项目目标

本项目不是一次按行数驱动的 dead-code cleanup，而是一次允许 breaking changes 的架构收敛。最终必须同时满足：

```text
one operation runtime owner
one operation admission path
one durable Rust-native session fact model
one typed product event stream
one snapshot/projection model
thin print / JSON / RPC / interactive adapters
operation-local capability snapshots
small stable crate facades
```

具体结果：

1. `pi-ai` 只负责 provider、model、auth、transport 和 streaming；生产路径不再依赖全局 provider registry。
2. `pi-agent-core` 只保留 Flow 驱动的 agent runtime；删除旧单体 loop 和公开 legacy entrypoint。
3. `pi-coding-agent` 的 runtime-affecting 行为全部经 `CodingAgentSession::run(Operation)` 或 control/query 入口进入统一 admission。
4. typed `ProductEvent` 是所有 adapter 的唯一实时语义边界，raw `AgentEvent`、`FlowEvent` 和 flat compatibility event 不向 adapter 泄露。
5. `SessionEvent` 继续是唯一 durable truth；旧 Rust-native session log 在 `0.2.0` 中仍可读取。
6. Snapshot cursor、reconnect、retention、overflow、terminal delivery 和 multi-client ownership 形成一个版本化协议。
7. plugin、tool 和 workflow 只接收 operation-local capability handles，不接收 raw runtime/session/provider services。
8. 删除 root-level、doc-hidden 和 deprecated 兼容表面，stable facade 成为唯一支持入口。

## 2. 非目标

- 不重写 Flow engine。
- 不把产品 session、ProductEvent 或 adapter 状态下沉到 `pi-agent-core`。
- 不把 coding-agent 产品语义放入 `pi-tui`。
- 不恢复 TypeScript session JSONL 兼容层。
- 不将 live ProductEvent 全量写入 session log。
- 不允许插件注册任意 Flow node 或获取 raw service 作为迁移捷径。
- 不以删除测试行数作为成果；边界护栏只能被等价或更强机制替代。
- 不要求旧 live RPC/UI clients 与 protocol major `2` 透明兼容；不兼容连接必须明确失败。

## 3. 当前基线与已知缺口

### 3.1 已具备的基础

- `CodingAgentSession::run(CodingAgentOperation)` 已支持 async、sync read-only 和 sync mutable dispatch。
- `CodingAgentOperation` 已覆盖 prompt、compact、branch summary、self-healing edit、agent/team invocation、plugin load/command、profile mutation、delegation、session navigation 和 export。
- `public_event.rs` 已有 Session/Profile/Agent/Team/Message/Tool/Runtime/Delegation/Workflow/Diagnostic/Capability event families。
- `SnapshotCoordinator` 已包含 client generation、takeover、retention/recovery 和 runtime lifecycle 基础。
- RPC、ProductEvent 和 UI Snapshot 已有独立 `1.0` protocol family 常量及协商逻辑。
- `SessionEventEnvelope` 已有 `session_sequence: Option<u64>`；manifest schema 为 v1，event schema 为 v2。
- capability snapshot 已进入 session admission、runtime build 和部分 workflow/service 路径。
- interactive 和 RPC 的大量 workflow 已经调用 `session.run(CodingAgentOperation::...)`。

### 3.2 尚未收敛的部分

- `OperationControl` 仍主要是单 active-operation guard，不是按 admission class 决策的 scheduler。
- query、control、runtime write、session write、root/child invocation 的规则仍散布在 facade、adapter 和 services。
- `CodingAgentEvent` 仍是内部兼容入口；部分 RPC 测试和转换路径直接构造它。
- internal `ProductEvent`、public `CodingAgentProductEvent` 和 adapter event conversion 仍有平行表面。
- TUI 仍直接调用 `ui_snapshot`、`view`、hydration 和部分 session internals；client-local 与 runtime-owned state 尚未完全隔离。
- session manifest/event validator 目前只接受单一版本，尚未形成显式 decoder matrix。
- production build 仍报告 replay/client/cancellation/snapshot 相关 dead-code warning，说明部分目标能力只有建模或测试路径。
- provider 测试夹具仍大量依赖 global registry guard，阻碍 product runtime 删除 test/debug compatibility bridge。

## 4. Breaking Release 策略

### 4.1 版本选择

所有活跃 crates 从 `0.1.0` 升到 `0.2.0`。在 `0.x` 阶段，minor bump 明确表示 Rust API breaking release。

协议版本独立管理：

| Contract | 当前 | 目标 | 策略 |
|---|---:|---:|---|
| Rust crate API | `0.1.0` | `0.2.0` | 删除 deprecated/doc-hidden 兼容面 |
| RPC protocol | `1.0` | `2.0` | 显式协商；v1 client 明确拒绝或进入独立 compatibility binary |
| ProductEvent protocol | `1.0` | `2.0` | typed family、统一 terminal/durability/operation association |
| UI Snapshot protocol | `1.0` | `2.0` | 加入 stream identity、完整 cursor 和 client/runtime state 分离 |
| Session manifest | v1 | v1 或 v2 | 只在 manifest shape 必须不兼容时升级 |
| Session event | v2 | v2 或 v3 | 保留 v2 decoder；只有 durable shape 必须变化时写 v3 |

live protocol 可以 breaking，durable history 不随 crate major 粗暴失效。旧日志必须通过 versioned decoder 读入统一内存模型，默认不原地重写。

### 4.2 支持窗口

- `0.1.x` 在 `0.2.0-rc.1` 创建后进入 maintenance-only，只接受安全和数据丢失修复。
- `0.2.0` 不保留 deprecated Rust symbols；迁移通过 guide 和 compile errors 完成。
- protocol v1 不在同一 runtime 内隐式降级。若必须短期支持，使用隔离的 `pi-coding-agent-v1` compatibility binary/branch，不在核心 runtime 保留双协议分支。
- session log v1/v2 的读取支持不设与 live protocol 相同的短窗口；删除 decoder 需要独立数据保留决策。

### 4.3 Breaking change 清单

`pi-ai`：

- 删除 root global `register`、`stream_model` 和 deprecated global built-in registration。
- 删除 `providers::anthropic::sse` 与 `transport::retry` 旧公开路径。
- 将 migration-private root modules 收窄；支持 API 只从 `pi_ai::api` 导入。
- `Agent`/runtime 必须注入 scoped `AiClient` 或 `ProviderRegistry`，不再读取全局 registry。

`pi-agent-core`：

- 删除 `agent_loop` compatibility module。
- 删除旧单体 `agent_turn_flow::runtime::run_loop` 和旧 request preparation。
- 从稳定表面移除具体 Flow node/free-function 导出，只保留 `Agent`、必要 Flow/runtime contracts 和 `api` facade。
- root re-export 不再作为支持路径；下游统一迁移到 `pi_agent_core::api`。

`pi-coding-agent`：

- 删除 root-level deprecated re-export，稳定入口仅为 `pi_coding_agent::api`。
- 删除已被 `run(Operation)` 取代的 broad workflow methods 和 adapter deep-service 入口。
- 删除 `CodingAgentProductEvent.family`、`kind` 字符串字段。
- 删除 flat `CodingAgentEvent` compatibility layer，typed ProductEvent 成为唯一实时产品事件。
- Snapshot/RPC wire 升至 major `2`；旧字段和隐式 reconnect 行为不保留。
- internal service、Flow、plugin registry 和 operation dispatch 类型不得从 stable facade 导出。

## 5. 交付模型

项目使用“里程碑 + vertical slice + deletion gate”。每个里程碑必须保持 workspace 可编译；任何兼容代码只有在 replacement consumer 和边界测试完成后才删除。

分支建议：

```text
new-main                         integration branch
release/0.1                     maintenance branch, created before first breaking merge
arch/operation-runtime          scheduler/admission work
arch/event-snapshot-v2          ProductEvent and Snapshot protocol work
arch/session-log-hardening      durable log compatibility work
release/0.2-rc                  release stabilization branch
```

每个 work package 使用独立 PR。跨里程碑 PR 不允许同时修改 durable schema、live protocol 和 public API。

## 6. 里程碑与工作包

## M0. Baseline、合同和 CI 恢复

目标：得到可重复、可比较的 breaking 前基线。

### WP0.1 恢复权威合同

- 恢复 `docs/product-event-contract.md`，或将 guard 改为引用明确的唯一权威合同。
- 校对 `architecture.md`、ProductEvent contract 和本计划的术语及版本规则。
- 添加 compatibility inventory：public symbols、wire fixtures、session fixtures、adapter commands。

### WP0.2 建立 API/protocol/session snapshots

- 保存 `pi_ai::api`、`pi_agent_core::api`、`pi_coding_agent::api` 的 public API baseline。
- 为 RPC v1、ProductEvent v1、UI Snapshot v1 保存 golden fixtures。
- 保存至少三类 session fixture：event v2 无 sequence、event v2 有 sequence、包含 incomplete/partial commit 的恢复样本。
- 保存 prompt、abort、reconnect、plugin reload、fork/export 的 deterministic faux-provider event traces。

### WP0.3 恢复强制 CI

- 让 `cargo check --workspace --all-targets` 可运行。
- 将 `cargo fmt`、Clippy `-D warnings`、workspace tests 和 boundary guards 设为 required checks。
- 将 TUI smoke 设为 adapter/event/snapshot 相关 PR 的 required check。
- 输出 dead-code suppression inventory；每项带 owner、reason、removal milestone。

退出条件：全量基线绿色；所有 breaking 前合同都有 fixture 或 API snapshot；不存在“先删后补测试”。

## M1. Provider Runtime 和 `pi-agent-core` 单路径化

依赖：M0。

### WP1.1 Scoped provider runtime

- 为 `Agent`/`AgentConfig` 定义 scoped provider runtime 输入，优先复用 `AiClient`/`ProviderRegistry`。
- 将 `ProviderStreamNode` 和 `ai_runtime` 从全局 `pi_ai::stream_model` 迁到 injected runtime。
- 更新 `pi-coding-agent::RuntimeService`，由 product owner 构造并注入 provider runtime。
- 用两个并行独立 registry 测试无串扰，覆盖 auth、model lookup 和 cancellation。

### WP1.2 删除旧 agent loop

- 删除旧 monolithic `runtime::run_loop`、专用 helpers 和 `PreparedProviderRequest`。
- 删除测试内复制的 `stream_options_for_turn`，改为 owning unit test 或 Agent observable test。
- 将使用 `PrepareContextNode`/`DecideStopOrToolsNode` 的集成测试迁成生产 `AgentTurnFlow` vertical slices；不为测试扩大 stable API。
- 删除 `agent_loop` wrapper 和对应只验证 legacy 隔离的测试。

### WP1.3 收窄 lower-level facade

- 下游全部迁移到 `pi_ai::api` 和 `pi_agent_core::api`。
- 删除 global registry helpers、旧 SSE/retry paths 和过度 Flow node exports。
- boundary guards 更新为验证新 facade，而不是删除 guard。

退出条件：一个 provider runtime 路径、一个 agent turn path；并发 scoped registries 无串扰；`pi-coding-agent -> pi-agent-core -> pi-ai` 依赖方向不变。

## M2. Unified Operation Admission 和 Scheduler

依赖：M1；可以与 M3 的 decoder-only 工作并行，但不得同时落同一 runtime contract PR。

### WP2.1 固化 operation contract

- 公开 `CodingAgentOperation` 的稳定字段只包含用户意图，不暴露 dispatch/service 细节。
- internal metadata 必须包含 `operation_id`、kind、origin/client、admission class、parent/root association、capability generation、runtime generation、idempotency key。
- 定义 `OperationOutcome` terminal status：completed、failed、aborted、rejected、in-doubt。
- 为每个 operation 建立 descriptor table test，禁止遗漏 class/durability/terminal family。

### WP2.2 引入真正的 scheduler

- 用 `OperationScheduler` 替代 `OperationControl` 的单 active guard。
- 实现 Query、ReadOnly、SessionWriteRoot、NonSessionRoot、RuntimeWrite、Child、Control admission classes。
- 明确 root slot、child lineage、runtime generation mutation、session write serialization 和 control priority。
- scheduler 返回 typed admission/rejection，不让 adapter 拼接 busy error string。
- cancellation token、prompt control 和 client ownership 绑定到 operation identity，不绑定到临时 task 地址。

### WP2.3 逐 vertical slice 迁移

按以下顺序迁移，每一项单独 PR：

1. Prompt + steer/follow-up/abort。
2. Compact + branch summary + self-healing edit。
3. Agent/team root invocation + delegated child invocation。
4. Plugin load/command + profile/settings runtime mutation。
5. Delegation approve/reject。
6. Fork/switch/export 和其他 session navigation/read operations。

每个 slice 同时迁移 interactive、print/JSON、RPC 和 public API tests。禁止 adapter 直接调用 `FlowService`、`SessionService`、`RuntimeService` 或 plugin services。

退出条件：所有 runtime-affecting intent 经一个 scheduler；control 在 backpressure/busy 时仍有优先级；root/child operation association 可由测试证明。

## M3. Durable SessionEvent Hardening

依赖：M0；最终接入依赖 M2 operation metadata。

### WP3.1 Versioned decoder matrix

- 将当前“版本必须等于常量”的 validator 改为显式 decoder dispatch。
- v2 event decoder 支持有/无 `session_sequence` 的历史日志。
- 若新字段可选且语义向后兼容，继续写 v2；只有 required shape 不兼容才引入 v3 writer。
- manifest 同理：能以 optional/additive 字段表达时保留 v1。
- unknown required feature fail closed；错误包含 schema、version、event id 和恢复建议，不输出敏感内容。

### WP3.2 Durable sequence 和 transaction

- 正式定义 `session_sequence` 单调性、append CAS、duplicate idempotency key 和 partial commit 语义。
- durable event 记录 operation/root/parent、capability/runtime generation 和 terminal status。
- startup scan 将 started-but-not-terminal operation 标记为 in-doubt，并写显式 recovery marker。
- fork/clone/export/replay 只从 SessionEvent/manifest 重建，不依赖 live ProductEvent 或 adapter state。

### WP3.3 故障注入验证

- 覆盖 append 前、append 后 manifest 更新前、terminal append 失败、重复重试、进程重启。
- 验证 completed/failed/aborted/recovered/in-doubt 可区分。
- 验证旧 fixture 可 replay、fork、clone、tree、export。

退出条件：旧日志可读；新日志 durable sequence 明确；partial commit 不会被误报为成功或静默丢失。

## M4. Capability Snapshot 闭环

依赖：M2 metadata；与 M3 的实现可部分并行。

### WP4.1 唯一权限语言

- admission 时冻结 `OperationCapabilitySnapshot`。
- provider/model 访问只接受 `ModelCapability`。
- filesystem/shell/tool 分别接受 narrow capability handles/set。
- plugin host 只接受 `PluginCapabilitySet`。
- durable read/write 分别接受 `SessionReadCapability`/`SessionWriteCapability`。
- 删除 operation/Flow/tool/plugin 对 raw auth、runtime、session store/service 的绕行访问。

### WP4.2 Generation 和 revocation

- RuntimeWrite 安装新 generation，不原地修改 active snapshot。
- `CapabilityChanged` 包含 generation 和 revocation policy。
- active operation 默认继续使用冻结 generation；需要强制撤销时产生显式 cancel/failure/revocation event。
- snapshot、ProductEvent 和 durable audit reference 使用同一 generation identity。

### WP4.3 清理 capability/plugin suppressions

- 移除 `capability_snapshot.rs` 文件级 `allow(dead_code)`，逐符号证明保留或删除。
- 保留已有 Lua/runtime 消费者的 tool/command/hook/UI/keybind 能力。
- Flow extension 需要单独产品决策：支持则限定声明式 extension point；不支持则先移除 loader/manifest/diagnostic consumer，再删除 trait。

退出条件：任一 operation 的权限决策可由 snapshot 解释；active operation 不静默热更新；plugin/tool 无 raw service capability escape。

## M5. ProductEvent v2、Snapshot v2 和 Adapter 收敛

依赖：M2、M3 terminal semantics、M4 generation semantics。

### WP5.1 唯一 typed ProductEvent

- 以现有 family enums 为基础，使 internal/public 共享一个 canonical typed model。
- 每个 event 统一带 stream identity、sequence、operation association、durability 和可选 terminal metadata。
- 建立 service/domain outcome 到 ProductEvent 的唯一 mapper。
- 禁止新增 `CodingAgentEvent` variant；先迁移 emitter，再迁移 consumer，最后删除 compatibility enum。
- 删除 deprecated `family`/`kind` string fields；wire 直接序列化 typed family/kind。

### WP5.2 Snapshot v2 contract

- cursor 至少包含 `stream_id`、last product sequence、capability generation、snapshot protocol version。
- 明确 snapshot 在 sequence N 包含的状态边界。
- 分离 client-local drafts、runtime-owned accepted/submitted operation、committed session projection。
- client generation/takeover 后旧 handle 必须失败；ack/detach/terminal receipt 有容量和幂等规则。

### WP5.3 Reconnect 和 backpressure

- retained range 覆盖 cursor 时 replay；否则返回 typed fresh-snapshot-required。
- bounded queue overflow 不伪装成普通 event；terminal/control/recovery 不能被 delta 挤掉。
- slow subscriber 明确 detach/reconnect；shutdown 有 drain boundary。
- 对 pending/live/replayed overlap 定义去重键和顺序。

### WP5.4 迁移所有 adapters

顺序：

1. print mode；
2. JSON mode；
3. RPC；
4. interactive/TUI。

每个 adapter 只能接收 Snapshot + ProductEvent + typed command outcome。TUI 不再直接调用 session hydration/runtime internals；RPC 不再构造 flat compatibility events。

退出条件：四类 adapter 的同一操作产生语义等价 trace；gap/reconnect/multi-client 测试通过；raw Flow/Agent/internal event 不出现在 protocol。

## M6. Public Facade 删除和模块拆分

依赖：M1--M5 全部退出条件。

### WP6.1 删除 compatibility APIs

- 执行第 4.3 节完整 breaking 清单。
- stable import fixture 只使用三个 crate 的 `api` facade。
- compile-fail fixture 验证 internal operation dispatch、services、Flow nodes、plugin registry 无法从 facade 导入。
- 生成 `0.1 -> 0.2` public API diff，并逐项关联 migration guide。

### WP6.2 拆分 `CodingAgentSession`

- 按 operation dispatch、connection/snapshot、query/view、lifecycle 和 test support 移动 impl。
- operation-specific business logic 放入 operation/service modules。
- facade delegation 保持显式，不使用宏掩盖 owner 或 side effect。
- 删除 replacement 已完成的 pass-through、test seam 和 file-level suppression。

### WP6.3 测试基础设施收敛

- 重复 EnvGuard/ProviderGuard 合并为一个 test-support owner。
- 手写 rustc negative runner 可迁到 `trybuild`，但失败能力必须等价。
- 源码扫描规则只有在 visibility、lint 或 CI check 接管后才能删除。
- wire、durability、association、ordering 和 boundary tests 保留。

退出条件：deprecated symbols 和 file-level dead-code suppression 为零；stable facade 小且可快照；无隐藏 fallback path。

## M7. `0.2.0` Release Train

依赖：M6。

### WP7.1 Alpha

- 升 crate versions 到 `0.2.0-alpha.1`，protocol constants 到 `2.0`。
- 发布 API migration guide、RPC/ProductEvent/Snapshot v2 spec、session compatibility matrix。
- 对实际 `0.1` session fixture 做只读 replay、fork、export soak test。

### WP7.2 Release Candidate

- `0.2.0-rc.1` 后冻结 public API、protocol schema 和 durable writer shape。
- 只接受 correctness、data loss、security、protocol ambiguity 和 migration blocker 修复。
- 运行长时 reconnect/backpressure、parallel clients、abort/retry 和 interrupted commit 测试。

### WP7.3 General Availability

- 发布 `0.2.0` 和完整 breaking changelog。
- 创建 `release/0.2`；保留 `release/0.1` maintenance policy。
- 发布后首个 patch 只处理兼容 decoder、数据恢复和明确回归，不恢复已删除 architecture path。

退出条件：所有 release gates 绿色；migration guide 覆盖每个 public API deletion；没有 P0/P1 未决数据或协议问题。

## 7. 依赖关系和关键路径

```text
M0 Baseline
  |
  +--> M1 Scoped provider + single agent loop
  |       |
  |       +--> M2 Operation scheduler/admission
  |               |
  |               +--> M4 Capability snapshots
  |
  +--> M3 SessionEvent decoder/hardening

M2 + M3 + M4
  |
  +--> M5 ProductEvent/Snapshot/Adapters v2
          |
          +--> M6 Facade deletion/module cleanup
                  |
                  +--> M7 0.2 release train
```

关键路径是 `M0 -> M1 -> M2 -> M4 -> M5 -> M6 -> M7`。M3 可以提前进行 decoder 和 fixture 工作，但 terminal/generation 字段接入必须等待 M2/M4 合同稳定。

## 8. PR 序列

建议的最小 review 单元如下：

1. 恢复 ProductEvent contract 和 all-targets baseline。
2. 添加 public API、protocol v1、session v1/v2 fixtures。
3. 注入 scoped `AiClient`/`ProviderRegistry`。
4. 删除 global provider runtime 调用。
5. 删除旧 monolithic agent loop。
6. 收窄 `pi-ai`/`pi-agent-core` facade。
7. 固化 operation descriptor/metadata table。
8. 引入 scheduler core 和 admission result。
9. 迁移 prompt/control slice。
10. 迁移 workflow slice。
11. 迁移 invocation/delegation slice。
12. 迁移 plugin/runtime-write/session-navigation slice。
13. 引入 versioned session decoder。
14. durable sequence/idempotency/recovery hardening。
15. capability handle 全路径接入。
16. canonical typed ProductEvent。
17. Snapshot v2 cursor/client state。
18. reconnect/backpressure hardening。
19. 迁移 print/JSON adapters。
20. 迁移 RPC v2 adapter。
21. 迁移 interactive/TUI adapter。
22. 删除 `CodingAgentEvent` 和旧 wire fields。
23. 删除 root/deprecated/doc-hidden compatibility surface。
24. 拆分 `CodingAgentSession` 和收敛 test support。
25. 版本升级、migration guide 和 `0.2.0-alpha.1`。

PR 8--12、16--21 必须附带 event-sequence/association tests；PR 13--14 必须附带旧 fixture 和故障注入 tests；PR 22--23 必须附带 API/protocol diff。

## 9. 验收矩阵

### 9.1 每个 PR

```bash
cargo fmt --all --check
cargo check -p <owning-crate>
cargo test -p <owning-crate> <focused-test-or-target>
git diff --check
```

### 9.2 每个里程碑

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

adapter、snapshot、event 或 terminal lifecycle 变更额外执行：

```bash
scripts/tui-smoke.sh
```

### 9.3 Release candidate 强制场景

| 场景 | 必须证明 |
|---|---|
| Prompt streaming | delta 顺序、terminal 唯一、operation ID 一致 |
| Abort/control | busy/backpressure 下可达，target identity 正确 |
| Root/child invocation | scheduler 不误拒绝，lineage 可追踪 |
| Plugin reload | generation 更新，active operation 不热变权限 |
| Reconnect in retention | 无重复、无缺口、cursor 前进 |
| Reconnect after eviction | 明确要求 fresh snapshot |
| Slow client | bounded memory，terminal/recovery 不被挤掉 |
| Multi-client takeover | stale handle 失败，client-local draft 隔离 |
| Partial commit | success 不误报，重启后 in-doubt/recovered 明确 |
| Old session logs | replay/fork/clone/tree/export 兼容 |
| Machine-readable modes | stdout/JSONL 无诊断污染 |
| Terminal lifecycle | resize/cursor/cleanup/scrollback 正常 |

## 10. Release Gates

### Gate A：允许开始删除 compatibility event/API

- 所有 production consumers 已迁移；
- public API/protocol diff 已生成；
- replacement tests 绿色；
- `rg`/CodeGraph 无 legacy caller；
- migration guide 已有对应条目。

### Gate B：允许升级 live protocol major

- v2 spec 和 fixtures 完整；
- negotiation 对 v1 明确失败；
- Snapshot + ProductEvent 可重建 adapter state；
- gap、overflow、terminal ordering 和 multi-client 行为均有测试。

### Gate C：允许升级 durable writer

- 旧 decoder 和 fixture tests 已存在；
- 不升级版本无法表达 required invariant；
- migration/read strategy 经过 recovery review；
- 不执行不可逆的原地批量重写。

### Gate D：允许发布 `0.2.0`

- workspace/Clippy/tests/TUI smoke 全绿；
- zero deprecated use、zero unexpected dead-code warning；
- old session compatibility matrix 全绿；
- release notes、migration guide、protocol specs 完整；
- 无 P0/P1 未解决问题。

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| scheduler 重写改变并发语义 | operation 被错误拒绝或并发写入 | admission table tests；逐 vertical slice 迁移 |
| event 迁移丢 terminal/association | UI 卡住、RPC 误报完成 | normalized terminal contract；adapter trace differential tests |
| snapshot cursor 定义不一致 | reconnect 重复或丢事件 | 明确 sequence boundary；retention/eviction property tests |
| durable schema 升级破坏旧日志 | 历史不可恢复 | versioned decoder；fixture corpus；禁止默认原地重写 |
| capability 收敛过度授权 | plugin/tool 越权 | narrow handles；negative capability tests；generation audit |
| 同时删除测试和实现 | 回归不可见 | replacement-first gate；禁止同 PR 无替代删护栏 |
| facade 大幅删除遗漏仓外调用 | 用户升级困难 | API diff；migration guide；alpha/RC feedback window |
| TUI 迁移引入生命周期回归 | 终端状态损坏 | virtual terminal tests + `scripts/tui-smoke.sh` |
| 双协议兼容污染新 runtime | 长期分支和隐式降级 | v1 独立 compatibility branch/binary；核心只实现 v2 |

## 12. 回滚策略

- M1--M4 的每个 vertical slice 可以按 PR revert，不能依赖数据库式不可逆 migration。
- live protocol v2 在 alpha/RC 阶段可以整体回退到上一个 RC；禁止在同一 protocol major 下改变既有字段语义。
- durable writer 在 RC 前默认保持当前版本；若启用新 writer，必须先发布支持新旧 decoder 的版本。
- 已写入的新 SessionEvent 不通过 history rewrite 回滚；回滚版本必须能读取它，或在发布前阻止 writer 上线。
- compatibility API 一旦在 GA 删除，不通过 patch release 恢复平行 architecture；只修 migration blockers 或提供外部 adapter。

## 13. 项目追踪指标

主指标：

- runtime-affecting adapter bypass 数量：目标 `0`；
- production global provider registry caller 数量：目标 `0`；
- flat compatibility event emitter/consumer 数量：目标 `0`；
- deprecated symbol/use 数量：目标 `0`；
- unexpected dead-code warning 和 file-level suppression 数量：目标 `0`；
- old session fixture replay success：目标 `100%`；
- protocol gap/overflow/terminal scenarios 覆盖率：目标 `100%` 场景覆盖；
- boundary guard 等价替代率：目标 `100%`，不得降低失败能力。

不作为主指标：净删除行数。替代合同、decoder 和 failure tests 可能增加代码；项目成功由路径唯一性、合同清晰度和恢复正确性衡量。

## 14. Definition of Done

当且仅当以下条件同时成立，architecture convergence 才完成：

1. `pi-ai`、`pi-agent-core`、`pi-coding-agent` 只有各自 `api` facade 是支持表面。
2. scoped provider runtime 替代所有 global registry production path。
3. `AgentTurnFlow` 是唯一 agent turn 实现。
4. 所有 runtime-affecting intent 经统一 scheduler/admission。
5. typed ProductEvent v2 是唯一 adapter event boundary。
6. Snapshot v2 支持明确的 multi-client、cursor、gap 和 backpressure 语义。
7. SessionEvent 是唯一 durable truth，旧日志仍可读。
8. operation-local capability snapshot 是唯一权限语言。
9. deprecated/legacy paths 已通过明确 deletion commits 删除，没有 fallback。
10. `0.2.0` release gates 全部通过，迁移文档可让 `0.1` 使用者完成升级。
