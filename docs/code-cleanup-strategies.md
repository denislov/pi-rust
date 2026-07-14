# `pi-rust` 代码清理方案

> 状态：基于 2026-07-15 工作区源码核对
>
> 输入：`pi-ai-code-review.md`、`pi-agent-core-code-review.md`、`pi-coding-agent-code-review.md`、`architecture.md`
>
> 目标：给出可落地的保守型与激进型清理路径，而不是直接执行删除

## 1. 结论摘要

三份 review 可以作为候选项清单，但不能直接作为删除清单。

- `pi-ai` 的 Anthropic SSE 副本确实没有仓内生产调用者，但它位于公开模块路径下。最稳妥的清理不是立即删除模块，而是把它改成指向唯一实现的兼容 re-export。
- `pi-agent-core` 的旧单体 `agent_turn_flow::runtime::run_loop` 及其专用 request 准备逻辑确实是可优先删除的内部死路径，这是三份报告里证据最充分、风险最低的一组。
- `PrepareContextNode` 并非“全仓无引用”。`tests/agent_turn_flow.rs` 在多个分段 Flow 测试中直接使用它；删除前必须先改造测试，不应与旧单体 loop 一起机械删除。
- `pi-coding-agent` 的 plugin command/hook/UI/keybind/flow-extension 接口不是死代码。它们由 `PluginService` 消费，并由 Lua plugin load、prompt hook、capability projection 和相关 Flow 使用。整组删除会破坏当前功能和架构基线。
- `capability_snapshot.rs` 也不是未消费的预留文件。`CodingAgentSession`、`RuntimeService`、`BranchSummaryService` 等已经使用 operation-local snapshot；文件级 `allow(dead_code)` 说明其中仍有未完成表面，不代表整文件可删。
- `CodingAgentEvent`、`ProductEvent` 和 `SessionEvent` 承担不同职责：兼容性内部事件、adapter-facing live event、durable fact。按 `architecture.md`，应先完成 family 化与 adapter 迁移，再删除兼容层，不能直接合并枚举。
- serde、public API、boundary/meta 测试不能仅因“测试宏行为”或“扫描源码”被判为低价值。只要字段名、负向可见性或分层约束是 wire/public/architecture contract，这些测试就是合同测试或架构护栏。

因此，建议默认采用保守型方案。激进型方案只有在明确接受 breaking release、完成 operation runtime 迁移并冻结协议版本后才成立。

## 2. 核对方法与当前基线

本次核对先使用仓库 `.codegraph/` 的调用图，再检查当前源码、测试引用和 `docs/architecture.md` 的规范性条款。判断“可删除”至少需要同时满足：

1. 仓内生产、测试、示例和 fixture 均无必要调用者；
2. 不属于 stable facade、公开兼容路径、序列化格式或持久化格式；
3. 不承载 `architecture.md` 明确要求的迁移目标或护栏；
4. 有可运行的基线验证，删除后能区分既有失败与回归。

当前验证状态：

- `cargo check --workspace` 通过，但 `pi-coding-agent` 仍产生 14 组 dead-code warning。它们主要集中在 product-event replay、client acknowledge/detach、compact cancellation 和 snapshot coordinator，三份 review 没有完整覆盖这些新候选项。
- `cargo check --workspace --all-targets` 失败。`crates/pi-coding-agent/tests/event_boundary_guards.rs` 引用了缺失的 `docs/product-event-contract.md`。
- 工作区当前文件整体显示为 untracked，无法用提交历史、`git blame` 或发布标签确认兼容窗口。因此任何公开路径删除都必须按潜在 breaking change 处理。

两套方案都必须先完成 Phase 0：恢复 `product-event-contract.md`（或修正其权威来源和 include 路径），保存一次全量验证结果，并把编译器 warning 形成逐符号清单。基线未恢复前不进行批量删除。

## 3. Review 逐项校正

| 候选项 | 当前代码上下文 | 核对结论 | 推荐处理 |
|---|---|---|---|
| `pi-ai/providers/anthropic/sse.rs` | 生产路径统一调用 `util::sse`；两个实现近似重复；旧路径仍是 `pub mod` | 实现重复属实，立即移除公开路径有兼容风险 | 保守：改成 re-export shim；激进：breaking release 删除模块 |
| `pi-ai/transport/retry.rs` | 只有一行 re-export；稳定 `api` facade 已直接导出同一组类型/函数 | 没有实现重复，但保留了旧公开路径 | 保守：保留并迁移仓内测试到 `api`；激进：删除旧路径 |
| `pi-ai` serde 测试 | 覆盖 `type`、`contentIndex`、model 字段名和 stop reason | 这些名称可能被下游序列化消费者观察 | 改成明确的 wire contract 断言；不要因使用 derive 而删除 |
| `pi-ai` retry/cost/registry 重复测试 | 部分输入集合重复，但测试层级不同 | 可合并数据表，不能同时丢掉实现测试和 facade 合同测试 | 每个合同保留一个 owning test 和一个 public-facade smoke test |
| `pi-agent-core` 旧 `runtime::run_loop` | 标记 `allow(dead_code)`，生产入口为 `AgentTurnFlow::run_state` | 真死代码 | 优先删除，连同仅为它服务的 helper/import |
| `PreparedProviderRequest` 和旧 `prepare_provider_request` | 只被上述旧 loop 使用；新 Flow 使用 nodes 实现 | 真死代码 | 随旧 loop 删除，保留 `stream_options_for_turn` |
| `PrepareContextNode` | 被 `tests/agent_turn_flow.rs` 多次组装进测试 Flow | review 的“无引用”结论错误 | 保守保留；激进时先把测试迁到生产图或内部测试 harness |
| `DecideStopOrToolsNode` | 生产图不用，但多个隔离行为测试使用 | 属测试 seam，不是无条件死代码 | 只有替代测试覆盖建立后才删 |
| `agent_loop` wrapper | 仓内生产无调用，但为 deprecated、doc-hidden 的公开模块 | 仓内可删不等于生态可删 | 保守保留并声明 removal release；激进 breaking release 删除 |
| 测试内 `stream_options_for_turn` 副本 | 集成测试复制私有实现 | 测试可能在验证自身副本，确有问题 | 将断言移到 owning unit test 或通过 `Agent` 可观察行为测试，不能为测试扩大公开 API |
| `pi-agent-core` meta/public/wire 测试 | 保护 facade、依赖方向、transcript 边界和 wire shape | 多项对应规范性边界 | 保留；仅在等价 lint/compile-fail test 上线后替换 |
| `pi-coding-agent/plugins/*` | `PluginService` 收集各 provider；Lua load 注册 command/UI/keybind/hook；prompt/runtime 使用结果 | “除 ToolProvider 外均未消费”是错误结论 | 禁止按 review 整目录删除 |
| `capability_snapshot.rs` | session admission、runtime build、branch summary 等使用 snapshot/capability handles | “整文件无消费者”是错误结论 | 继续收敛为唯一权限语言；只删逐符号证明无用的叶子 |
| `session_service.rs` 的 allow 项 | 混合了旧 wrapper、snapshot-aware replacement、replay/recovery 和测试观测点 | 不能按 attribute 计数批量删除 | 按“旧方法 -> 新方法”调用迁移逐项处理 |
| `event_bridge::last_sequence` | 生产未调用，但多项单元测试用它验证 cursor 幂等性 | 有测试观测价值 | 可改为 `cfg(test)`，不应删除对应不变量测试 |
| 根级 deprecated re-export | binary、示例和多项集成测试仍在使用 | 迁移尚未完成 | 先迁移仓内调用，再经过弃用窗口删除 |
| `family`/`kind` deprecated 字段 | 仍被构造并有测试直接读取，可能影响 JSON/外部消费者 | 仍是兼容合同 | 先迁移消费者并版本化协议，再删除 |
| `CodingAgentEvent` | EventService、Flow、RPC/JSON/interactive adapter 转换链仍使用 | 活跃兼容层 | 按架构 Stage 2 family 化，不做直接合并 |
| `pi-coding-agent` meta tests | 强制 operation/service/event/plugin/UI/public API 边界 | 与 `architecture.md` 的 must/contract 对齐 | 不成批删除；可换实现方式但保持同等失败能力 |
| `CodingAgentSession` pass-through | 大文件问题属实，但宏 delegation 会隐藏 ownership | 需要按 operation/service 边界拆分，而不是只压缩行数 | 移动实现到 focused modules；保持 facade 薄而显式 |

## 4. 方案 A：保守型清理

### 4.1 适用条件

- 当前版本仍可能被仓外代码嵌入；
- 不准备发布 breaking API/protocol change；
- 目标是低风险降低重复和 warning，而不是一次性达到最终架构；
- 允许保留很薄的 deprecated compatibility shim。

### 4.2 实施阶段

#### A0. 恢复基线和建立清理台账

1. 恢复或重建缺失的 `docs/product-event-contract.md`，确认它与 `architecture.md` 的 ProductEvent 章节没有相互冲突。
2. 运行全量检查并记录现有 warning。对每个 `allow(dead_code)` 或编译器 warning 标注为：真实死代码、兼容 shim、测试 seam、迁移目标、条件编译路径。
3. 为公开路径建立清单：stable `api`、deprecated root export、doc-hidden public module、JSON/RPC wire、session log schema。
4. 明确清理 PR 规则：一类行为一个 PR；删除 PR 不夹带事件协议或 scheduler 重构。

退出条件：`cargo check --workspace --all-targets` 可运行，且既有失败已书面记录。

#### A1. 清理 `pi-ai` 的实现重复，不破坏路径

1. 将 `providers/anthropic/sse.rs` 从完整副本改为对 `util::sse::{ServerSentEvent, process_chunk, iterate_sse}` 的 re-export，保留 `providers::anthropic::sse` 路径。
2. 把 SSE 行为测试集中到 `util::sse`，在旧路径只保留一个可导入/类型一致性 smoke test（如确有兼容承诺）。
3. 保留 `transport::retry` shim；将仓内 `transport_contract.rs` 的推荐路径迁移到 `pi_ai::api`，并用 API boundary test 明确 shim 是临时兼容面。
4. 合并 retry 的重复输入表，但保留 public facade 可用性和错误边界测试。
5. 保留 serde wire 测试，将测试名改成“wire shape remains stable”，使用结构化 `serde_json::Value` 或完整 JSON 断言，避免脆弱的 `contains`。

预期：净减约 150--180 行重复实现，不删除公开符号。

#### A2. 删除 `pi-agent-core` 已证明的内部死路径

1. 删除 `agent_turn_flow/runtime.rs` 中旧单体 `run_loop` 及其专用 helper。
2. 删除 `loop_runtime/context.rs` 的 `PreparedProviderRequest` 和旧 `prepare_provider_request`，保留并继续测试 `stream_options_for_turn`。
3. 删除随旧路径失效的 import、`allow(dead_code)` 和重复 helper。
4. 将 `queues_thinking.rs` 中复制的 `stream_options_for_turn` 测试迁入 owning module，或改成通过 `Agent` 的可观察 request options 验证。
5. 暂时保留 `PrepareContextNode`、`DecideStopOrToolsNode` 和 `agent_loop` compatibility wrapper。为 wrapper 写清移除条件，不新增调用者。

预期：净减约 900 行内部生产代码，且不改变 stable facade。

#### A3. 收紧 `pi-coding-agent`，不删除正在迁移的架构

1. 迁移 binary、examples 和集成测试中的 deprecated 根级导入到 `pi_coding_agent::api`；兼容 re-export 本阶段仍保留。
2. 逐项处理当前 14 组 compiler warning。优先删除没有测试、没有架构目标、没有协议语义的私有叶子；对 replay/client/snapshot/cancellation 项先核对 `architecture.md` Stage 3/6，不以 warning 为删除依据。
3. 把仅供内部单元测试读取的观测方法改成 `#[cfg(test)]`，例如确认 `last_sequence` 只用于同模块测试后再收窄。
4. 对 `SessionService` 采用 replacement pair 清理：调用者全部迁移到 snapshot-aware 方法后，才删旧 wrapper；replay/recovery 方法必须先确认启动恢复和旧日志兼容。
5. 按职责拆分 `coding_session/mod.rs` 的实现块，例如 connection/snapshot、operation dispatch、session navigation、test support。只移动代码，不改公开签名、事件顺序和 side-effect owner。
6. 保留 plugin traits、capability snapshots、三层事件模型和所有对应 boundary guards。

预期：主要收益是 warning 降低、owner 清晰和仓内 deprecated 调用清零；不承诺大规模净删行数。

### 4.3 保守型完成标准

- stable `api`、旧兼容路径、JSON/RPC 输出和 Rust-native session log 保持兼容；
- `pi-agent-core` 生产入口只剩 Flow 路径，但 legacy wrapper 仍能转发；
- Anthropic SSE 只有一个实现；
- plugin command/hook/UI/keybind/dialog、capability snapshot 和 adapter 行为不回退；
- `cargo fmt --all --check`、Clippy、全量测试通过；涉及 interactive projection 时额外通过 `scripts/tui-smoke.sh`。

## 5. 方案 B：激进型清理

### 5.1 适用条件

- 明确安排 breaking release，或者确认没有仓外消费者；
- 可以同时修改 API、协议版本、fixtures、示例和所有 adapters；
- 团队接受先完成 `architecture.md` 的 operation runtime 收敛，再删除兼容面；
- 目标是减少并行抽象和公开表面，而不只是追求删除行数。

激进型不是“把 review 中标为 dead/meta 的文件全部删掉”。正确顺序是先建立替代合同，再删除旧合同。

### 5.2 实施阶段

#### B0. 冻结合同并声明 breaking 范围

1. 完成 A0，并为 public API、ProductEvent protocol、Snapshot cursor、SessionEvent schema 分别确定版本策略。
2. 用 fixture 或 API snapshot 记录 breaking 前表面；列出明确删除项和迁移说明。
3. 建立 performance/behavior baseline：prompt streaming、abort/control、reconnect gap recovery、plugin reload、session replay/fork/export、print/JSON/RPC stdout cleanliness。

#### B1. 先完成 operation runtime 收敛

1. 让所有 runtime-affecting adapter 请求进入 `IntentRouter -> CodingAgentSession::run(Operation)`。
2. 用明确的 admission class/scheduler 决策替换散落的 busy check；保持 operation ID、async cancellation 和 parent/child association。
3. 让 Flow/operation 只接收 operation-local capability/runtime snapshot，side effect 继续由 service 持有。
4. 将 `CodingAgentSession` 的 workflow 实现迁到 focused operation/service modules；facade 只保留 run/control/subscribe/capabilities/snapshot/open-close 等稳定 verbs。

删除门槛：所有 adapter 和 public API 测试均不再调用 broad workflow methods，且 operation association、cancel 和 durable write 测试通过。

#### B2. 收敛事件与 snapshot 模型

1. 按 `Workflow/Agent/Team/Tool/Session/Plugin/Delegation` family 引入内部 typed ProductEvent。
2. 建立 `CodingAgentEvent -> typed ProductEvent` 的唯一转换，并逐个迁移 print、JSON、RPC、interactive adapter。
3. 固化 `(stream_id, sequence)`、retention gap、fresh snapshot、backpressure 和 terminal event 语义。
4. 保持 `SessionEvent` 为唯一 durable truth；不要把 live ProductEvent 全量持久化，也不要让 UiState 反写 session log。
5. 所有 adapter 迁移完成、兼容协议版本结束后，删除 flat `CodingAgentEvent` 兼容层以及只保护该层的转换代码。

删除门槛：snapshot + ProductEvents 能重建 UI/RPC 状态；旧 Rust-native 日志仍可读取；协议不兼容时显式拒绝而非静默降级。

#### B3. 删除公开兼容表面

在 breaking release 中统一删除，而不是零散删除：

- `pi-ai::providers::anthropic::sse` 旧路径；
- `pi-ai::transport::retry` 旧路径，保留 `pi_ai::api`；
- `pi-agent-core::agent_loop` wrapper；
- 不属于 stable facade 的过度导出 Flow nodes/functions；
- `pi-coding-agent` 根级 deprecated re-export；
- `CodingAgentProductEvent.family/kind` deprecated fields，在协议迁移完成后删除。

删除前应运行 public API diff；即使 crate 版本当前为 `0.1.0`，也不要把 doc-hidden `pub` 自动视作私有。

#### B4. 重构 plugin/capability 表面，而非整组删除

1. 保留已经有 Lua/runtime 消费者的 command/hook/UI/keybind/tool 能力。
2. 若决定不支持 arbitrary Flow extension，先从 manifest/loader/registry/diagnostic 中移除该能力，再删 trait；否则保留声明式 extension point，禁止向插件暴露 raw Flow/service。
3. 删除 `capability_snapshot.rs` 中被 operation-local capability model 取代的旧类型，但保留 generation、revocation、narrow handles 和审计引用。
4. 去掉文件级 `allow(dead_code)`，改为逐符号处理；编译器应重新成为未消费代码的探测器。

#### B5. 替换高维护测试，不降低约束

1. 用 `trybuild` 或等价 compile-fail harness 替换手写 `rustc` fixture runner，但保留“内部 service/Flow/plugin 类型不可从 stable API 导入”的负向测试。
2. 将可表达为编译边界的源码扫描迁成 visibility/module boundary；将格式化、命名和禁止 sleep 等规则迁成 lint/CI script。
3. 只有等价护栏在 CI 中生效后，才删除原 meta test。
4. 合并重复数据表和 test support；wire shape、session replay、operation association、adapter ordering 和 architecture direction 测试不得因“行数多”而删除。

### 5.3 激进型完成标准

- stable facade 收敛为 operation runtime verbs，仓内不存在 deprecated compatibility 调用；
- 所有 runtime-affecting adapter 走统一 admission；
- adapter 只消费 typed ProductEvent + Snapshot，不消费 raw FlowEvent/AgentEvent；
- SessionEvent replay/fork/export 与旧日志兼容；
- plugin capability 不向下层 crate 泄露产品类型，capability generation 在 operation 生命周期内稳定；
- 删除旧路径后，public API diff、protocol fixtures、全量测试、Clippy 和 TUI smoke 全部通过。

激进方案不应承诺 review 所称的“约 5,800 行”净删除。plugin/capability/meta-test 的大部分内容不能无替代删除，operation/event 迁移还会新增代码。更合理的指标是：并行 runtime 路径归一、deprecated 表面归零、文件级 dead-code suppression 归零、边界测试失败能力不下降。净行数只作为结果统计，不作为验收目标。

## 6. 推荐执行顺序

建议按以下 PR 顺序执行，每个 PR 独立可回滚：

1. `chore(test): restore product event contract baseline`
2. `refactor(pi-ai): canonicalize SSE implementation`
3. `refactor(pi-agent-core): remove retired monolithic turn loop`
4. `test(pi-agent-core): stop testing copied stream option logic`
5. `refactor(pi-coding-agent): migrate internal users to stable facade`
6. `chore(pi-coding-agent): classify and remove proven private dead leaves`
7. `refactor(pi-coding-agent): split coding session implementation by ownership`

到第 7 步为止属于保守型方案。若选择激进型，再继续 operation admission、event family、snapshot convergence、public compatibility removal 和 guard replacement；不要把这些跨合同变更塞进同一个 PR。

## 7. 每阶段验证矩阵

最小验证按 owner 运行：

```bash
cargo fmt --all --check
cargo check -p pi-ai
cargo test -p pi-ai
cargo check -p pi-agent-core
cargo test -p pi-agent-core agent_turn
cargo check -p pi-coding-agent
cargo test -p pi-coding-agent --test public_api
cargo test -p pi-coding-agent --test product_event_contract
cargo test -p pi-coding-agent --test operation_association
```

跨 crate/API/event/snapshot/plugin 改动完成后运行：

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

涉及 interactive adapter、event bridge、snapshot hydration 或终端生命周期时运行：

```bash
scripts/tui-smoke.sh
```

额外人工核对项：

- print/JSON/RPC 的 stdout 保持机器可读，诊断只进入 stderr 或结构化事件；
- abort、steer、follow-up 和 reconnect 不丢 operation association；
- provider fake credentials 和临时 `PI_RUST_DIR` 仍隔离真实用户配置；
- 新旧 session replay、fork/clone/tree/export 结果一致；
- plugin reload 后 capability generation 更新，但活动 operation 不静默热更新权限。

## 8. 最终建议

当前应采用方案 A。它能安全拿掉约一千行已证明的重复/内部死实现，同时保留公开兼容、plugin capability、event/persistence 和架构护栏。

方案 B 应作为 architecture convergence 项目执行，而不是 cleanup sprint。启动条件是：Phase 0 基线恢复、breaking release 获批、operation runtime 的替代合同已落地。否则按原 review 的激进删除建议执行，最可能造成的不是“代码更干净”，而是 plugin 功能缺失、adapter 协议回归、旧日志不可恢复以及公开路径的无预警破坏。
