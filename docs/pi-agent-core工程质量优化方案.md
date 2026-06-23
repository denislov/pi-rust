# pi-agent-core 工程质量优化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在保持 TypeScript `pi/packages/agent` 行为兼容和现有测试通过的前提下，把 `pi-agent-core` 从迁移期的集中式实现，逐步优化为边界清晰、错误可诊断、状态收敛、便于继续扩展的 Rust 核心 crate。

**Architecture:** 采用 strangler-style incremental refactor：先补行为护栏，再把 `agent_loop.rs`、`harness.rs`、`types.rs` 的职责拆到更小的内部模块，最后收紧错误模型和 public API。每个阶段都保持公开 API 尽量兼容，通过 re-export 保持调用方迁移成本可控。

**Tech Stack:** Rust 2024, `tokio`, `futures`, `async-stream`, `tokio-util::CancellationToken`, `serde`, `thiserror`, `pi-ai`, deterministic offline tests with faux providers.

---

## 结论先行

`pi-agent-core` 当前工程质量不差。它已经完成了 agent loop、tool calling、hooks、parallel tools、session JSONL、resources、compaction、harness、branch summary、proxy、shell output 等一批真实能力，并且有比较完整的离线测试覆盖。

真正的问题不是“代码不可用”，而是迁移速度带来的结构性压力已经开始集中在几个文件：

- `crates/pi-agent-core/src/agent_loop.rs`：859 行，单个 `run_loop` 同时负责 turn lifecycle、队列、compaction、context conversion、provider request、LLM stream、tool execution、hooks、终止逻辑。
- `crates/pi-agent-core/src/harness.rs`：985 行，同时定义 harness 类型、事件、subscription、hook registry、prompt lifecycle、provider hook composition、stream option patch。
- `crates/pi-agent-core/src/types.rs`：656 行，同时承载配置、消息、事件、工具、资源、compaction、stream alias。
- `crates/pi-agent-core/src/agent.rs`：`AgentState` 是共享可变大对象，`Agent` 通过 `Arc<RwLock<AgentState>>` 暴露多个运行时写入口。

这些选择在迁移期合理，因为能快速保持 TS parity。但如果继续按当前形态追加 session tree、plugin hooks、tool sandbox、provider observability、TUI integration，核心循环和 harness 会越来越难审阅，也更容易引入行为回归。

推荐路线不是推倒重写，而是分阶段收敛：

1. 先建立测试护栏，锁定当前兼容行为。
2. 抽出 agent loop 内部的 request preparation、tool execution、turn control。
3. 把状态修改收敛到 `AgentState` 方法，减少任意模块直接读写字段。
4. 拆分 `types.rs` 和 `harness.rs`，通过 re-export 保持 public API 稳定。
5. 引入结构化 agent loop error，逐步替换 `String` 错误。
6. 清理测试 warning 和迁移中间态代码。

## 非目标

- 不修改 `pi/` TypeScript reference repo。
- 不改变 `pi-ai` provider 行为。
- 不在第一轮中改变 `Agent::prompt`、`Agent::run`、`AgentHarness::prompt` 的外部调用方式。
- 不把 `pi-agent-core` 拆成多个 crate。当前问题可以先通过模块边界解决。
- 不引入复杂 actor runtime 或大型框架。先收敛已有状态和函数边界。
- 不要求真实 provider key 参与测试。

## 当前设计优点

### 测试基础较好

已经覆盖：

- `tests/agent_loop.rs`：单轮响应、工具调用、abort、provider error、max turns、tool update。
- `tests/hooks.rs`：before/after tool hooks、transform context、convert_to_llm、prepare_next_turn。
- `tests/parallel_tools.rs`：parallel/sequential 工具执行顺序和性能差异。
- `tests/queues_thinking.rs`：steer/follow-up 队列、thinking level。
- `tests/session_wire.rs`、`tests/session_jsonl.rs`、`tests/session_repo.rs`、`tests/session_context.rs`：JSONL session 形状、repo、context rebuild。
- `tests/resources.rs`、`tests/sourced_resources.rs`：skills 和 prompt templates。
- `tests/m9_harness.rs`、`tests/harness_subscribe.rs`：harness lifecycle、subscription、provider hooks。
- `tests/m9_branch_proxy_shell.rs`：branch summary、proxy stream、shell capture、truncation。

本方案审阅时已验证：

```bash
cargo test -p pi-agent-core
```

结果：全部测试通过。存在若干测试侧 unused/dead-code warnings，需要后续清理。

### TS parity 意识明确

多个文件保留了对应 TS 文件的注释引用，例如：

- `crates/pi-agent-core/src/agent_loop.rs` 对应 `pi/packages/agent/src/agent-loop.ts`。
- `crates/pi-agent-core/src/harness.rs` 对应 `pi/packages/agent/src/harness/agent-harness.ts` 和 `types.ts`。
- `crates/pi-agent-core/src/session/*` 对应 TS session storage/repo/context。

迁移阶段这很重要。优化时应保留这些引用，但要避免把 TS 的大文件结构原样固化为 Rust 长期结构。

### 部分领域已具备模块边界

相对健康的目录：

```text
crates/pi-agent-core/src/session/
crates/pi-agent-core/src/resources/
crates/pi-agent-core/src/compaction/
```

这些目录说明当前实现不是随意堆叠。后续优化应沿用这种方向，把 loop、harness、types 也拆成类似的领域模块。

## 主要问题

### 1. `agent_loop.rs` 是最大复杂度集中点

文件：`crates/pi-agent-core/src/agent_loop.rs`

当前 `run_loop(state: Arc<RwLock<AgentState>>) -> AgentStream` 同时处理：

- cancel 和 max turns。
- `TurnStart` event。
- steering queue drain。
- runtime compaction。
- `transform_context` hook。
- `convert_to_llm` hook。
- context assembly。
- thinking options。
- provider request override。
- before provider request hook。
- `pi_ai::stream_model`。
- LLM event passthrough。
- assistant message append。
- stop/error/aborted/tool_use stop reason。
- tool call discovery。
- per-tool sequential override。
- sequential tool execution。
- parallel tool execution。
- before/after tool hook。
- tool update streaming。
- tool result append。
- follow-up queue continuation。
- `prepare_next_turn` hook。

这导致两类风险：

- 行为风险：新增一个 hook 或 event 时，容易打破 tool、queue、compaction、turn 之间的顺序。
- 维护风险：测试失败时很难快速定位是 request preparation、stream handling、tool execution 还是 state mutation。

### 2. `AgentState` 是共享可变大对象

文件：`crates/pi-agent-core/src/agent.rs`

当前状态字段：

```rust
pub struct AgentState {
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub config: AgentConfig,
    pub cancel_token: CancellationToken,
    pub steering_queue: VecDeque<AgentMessage>,
    pub follow_up_queue: VecDeque<AgentMessage>,
    pub(crate) provider_request_override: Option<ProviderRequestOverride>,
}
```

`agent_loop.rs`、`agent.rs`、`harness.rs` 会直接或间接修改这些字段。问题不是 `RwLock` 本身，而是缺少状态 API：

- 没有统一的 `next_message_id`。
- `steer()` / `follow_up()` 用队列长度生成 id，drain 后可能重复。
- `provider_request_override` 的消费语义隐藏在 loop 内部。
- `config` 可以被 hook 修改，也可以被 harness 修改，边界不明显。
- 多处 `read().unwrap()` / `write().unwrap()` 让 poison 后直接 panic。

### 3. 错误模型不统一

文件：

- `crates/pi-agent-core/src/errors.rs`
- `crates/pi-agent-core/src/hooks.rs`
- `crates/pi-agent-core/src/types.rs`
- `crates/pi-agent-core/src/agent_loop.rs`

当前已经有结构化错误：

- `FileError`
- `ExecutionError`
- `AgentHarnessError`
- `BranchSummaryError`

但 loop、hooks、tool function 仍大量使用 `String`：

```rust
pub type HookFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;
pub type ToolFn = Arc<dyn Fn(...) -> Pin<Box<dyn Future<Output = Result<AgentToolOutput, String>> + Send>> + Send + Sync>;
```

结果是上层只能看到 `"LLM error"`、`"aborted"`、`"unknown tool: x"` 这类文本，无法稳定区分：

- invalid state
- busy
- aborted
- provider error
- hook error
- tool blocked
- tool execution failed
- compaction failed
- stream ended without Done

### 4. `types.rs` 变成 public model 聚合桶

文件：`crates/pi-agent-core/src/types.rs`

它包含：

- `ThinkingLevel`
- `ToolExecutionMode`
- `QueueMode`
- `AgentToolOutput`
- `AgentToolResult`
- `Skill`
- `PromptTemplate`
- resource diagnostics
- `CompactionSettings`
- `CompactionConfig`
- `AgentMessage`
- `ToolFn`
- `AgentTool`
- `AgentConfig`
- `ProviderRequestSnapshot`
- `AgentEvent`
- `AgentStream`

短期方便，长期问题是：

- 任意领域变化都会触碰同一个文件。
- public API 暴露面难以审查。
- `AgentMessage`、`AgentEvent`、`AgentTool`、resources、compaction 混在一起，不利于 domain ownership。
- 测试也混在这个文件底部，进一步增加上下文长度。

### 5. `harness.rs` 职责接近上限

文件：`crates/pi-agent-core/src/harness.rs`

当前同时承载：

- hook type aliases。
- patch structs。
- provider auth types。
- harness events。
- lifecycle phase。
- subscription guard。
- typed `on` registry。
- `AgentHarness` struct。
- prompt lifecycle。
- `map_agent_event`。
- provider request hook composition。
- provider stream hook composition。
- stream options patch application。
- header merge/delete。

这和 TS `agent-harness.ts` 的大文件相似，但 Rust 中继续维持单文件会让后续 plugin/hook/session 能力难以维护。

### 6. 少量迁移中间态代码需要清理

示例：

- `AgentHarnessPhase::Compaction` 和 `BranchSummary` 当前保留但不可达。
- `AgentHarness::prompt` 中 `config` 计算逻辑当前始终为 `None`。
- 测试中存在 unused imports、dead fixture warnings。

这些不是严重 bug，但会削弱代码信号质量。

## 目标架构

优化后的 crate 仍保持当前 crate 边界，但内部文件结构建议演进为：

```text
crates/pi-agent-core/src/
  agent.rs
  lib.rs

  loop_runtime/
    mod.rs
    context.rs
    control.rs
    error.rs
    events.rs
    state.rs
    tools.rs

  harness/
    mod.rs
    events.rs
    hooks.rs
    provider.rs
    subscription.rs
    patch.rs
    phase.rs

  types/
    mod.rs
    config.rs
    event.rs
    message.rs
    resource.rs
    tool.rs

  session/
  resources/
  compaction/
```

说明：

- `loop_runtime` 是内部模块，不必公开给调用方。
- `types/mod.rs` 负责 re-export，保持 `pi_agent_core::types::AgentMessage` 等路径可继续工作。
- `harness/mod.rs` 负责 re-export，保持 `pi_agent_core::harness::AgentHarness` 和 root `pub use harness::{...}` 兼容。
- `agent.rs` 保持 `Agent` facade，但状态操作逐步下沉到 `AgentState` 方法。

## 阶段 0：建立重构护栏

### 目的

先补 characterization tests 和 warning cleanup，避免后续拆文件时误改行为。

### 文件范围

- Modify: `crates/pi-agent-core/tests/agent_loop.rs`
- Modify: `crates/pi-agent-core/tests/hooks.rs`
- Modify: `crates/pi-agent-core/tests/parallel_tools.rs`
- Modify: `crates/pi-agent-core/tests/harness_subscribe.rs`
- Modify: `crates/pi-agent-core/tests/common/mod.rs`
- Modify: `crates/pi-agent-core/tests/resources.rs`
- Modify: `crates/pi-agent-core/tests/session_harness_entries.rs`

### 任务

- [ ] 清理测试 warning：删除 `tests/resources.rs` 中未使用的 `std::path::PathBuf` import。
- [ ] 清理测试 warning：删除 `tests/session_harness_entries.rs` 中未使用的 `serde_json::Value` import。
- [ ] 如果 `tests/common/mod.rs` 的 faux provider 只被部分 integration test 使用，把未使用的 helper 移到实际使用它们的测试文件，或者给 test helper 模块添加局部 `#![allow(dead_code)]` 并注明原因。
- [ ] 在 `tests/agent_loop.rs` 增加并发调用行为测试，明确当前 busy 行为是 panic 还是结构化错误。此阶段不改行为，只锁定。
- [ ] 在 `tests/queues_thinking.rs` 增加 queue id 重复的 characterization test，记录当前 `steer_0` / `followup_0` 在 drain 后可能重复的事实。后续阶段改行为时同步更新测试。
- [ ] 在 `tests/hooks.rs` 增加 hook error 分类测试，当前先断言 string error 被 `AgentEvent::AgentError` 携带。

### 建议测试片段

新增到 `crates/pi-agent-core/tests/queues_thinking.rs`：

```rust
#[test]
fn steering_message_ids_repeat_after_queue_is_drained_current_behavior() {
    let agent = Agent::new(config_with_faux_model());
    agent.steer("first");
    let first = agent.drain_steering_queue();
    agent.steer("second");
    let second = agent.drain_steering_queue();

    assert!(matches!(
        &first[0],
        AgentMessage::UserText { message_id, .. } if message_id == "steer_0"
    ));
    assert!(matches!(
        &second[0],
        AgentMessage::UserText { message_id, .. } if message_id == "steer_0"
    ));
}
```

实际落地时使用该测试文件已有的 model/config helper 名称。如果当前 helper 不可复用，添加一个本地 helper：

```rust
fn config_with_faux_model() -> AgentConfig {
    AgentConfig::new(pi_ai::types::Model {
        id: "faux".into(),
        name: "faux".into(),
        api: "faux".into(),
        provider: "faux".into(),
        base_url: "".into(),
        reasoning: false,
        input: vec![],
        cost: pi_ai::types::ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
        headers: None,
    })
}
```

如果 `Model` 字段和当前 `pi-ai` 定义不一致，以 `tests/common/mod.rs` 里的 `faux_model` 为准，不在生产代码中新增默认 model。

### 验收标准

```bash
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- `cargo test -p pi-agent-core` 全部通过。
- 测试 warning 数量减少；如果保留 `allow(dead_code)`，必须只出现在 test helper 模块。
- 没有生产代码行为变化。

## 阶段 1：拆出 loop request preparation

### 目的

把 `run_loop` 中 provider request 准备逻辑拆成独立模块，降低主循环复杂度。

### 文件范围

- Create: `crates/pi-agent-core/src/loop_runtime/mod.rs`
- Create: `crates/pi-agent-core/src/loop_runtime/context.rs`
- Create: `crates/pi-agent-core/src/loop_runtime/control.rs`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Test: `crates/pi-agent-core/tests/hooks.rs`
- Test: `crates/pi-agent-core/tests/queues_thinking.rs`

### 设计

新增内部 request preparation 类型：

```rust
pub(crate) struct PreparedProviderRequest {
    pub model: pi_ai::types::Model,
    pub context: pi_ai::types::Context,
    pub stream_options: pi_ai::types::StreamOptions,
}
```

新增函数：

```rust
pub(crate) async fn prepare_provider_request(
    state: &std::sync::Arc<std::sync::RwLock<crate::agent::AgentState>>,
    cancel: tokio_util::sync::CancellationToken,
    transformed_messages: Option<Vec<crate::types::AgentMessage>>,
    llm_messages_override: Option<Vec<pi_ai::types::Message>>,
) -> Result<PreparedProviderRequest, String>
```

职责：

- 根据 `transformed_messages` 和 `llm_messages_override` 调用 `assemble_context` 或 `convert_to_context`。
- 根据 model reasoning 和 `ThinkingLevel` 设置 `StreamOptions.thinking`。
- 消费 `provider_request_override`。
- 始终设置 `stream_options.cancel = Some(cancel)`。

仍留在 `agent_loop.rs` 的职责：

- 调用 transform hook。
- 调用 convert hook。
- 调用 before provider request hook。
- yield `AgentEvent::BeforeProviderRequest`。
- 调用 `pi_ai::stream_model`。

### 步骤

- [ ] 新增 `loop_runtime/mod.rs`，只导出内部模块：

```rust
pub(crate) mod context;
pub(crate) mod control;
```

- [ ] 在 `lib.rs` 中加入内部模块：

```rust
mod loop_runtime;
```

不要 `pub mod loop_runtime`，避免暴露尚未稳定的内部结构。

- [ ] 把 `agent_loop.rs` 中 251 到 316 行的 context/options/model/override 准备逻辑移动到 `loop_runtime/context.rs`。
- [ ] 在 `agent_loop.rs` 中用 `prepare_provider_request(...)` 替换原内联代码。
- [ ] 保持 `before_provider_request` hook 的调用位置不变。
- [ ] 保持 `AgentEvent::BeforeProviderRequest` 的 payload 形状不变。

### 验收标准

```bash
cargo test -p pi-agent-core --test hooks
cargo test -p pi-agent-core --test queues_thinking
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- `transform_context_hook_rewrites_messages_before_llm_call` 通过。
- `convert_to_llm_hook_overrides_default_message_conversion` 通过。
- `thinking_level_sets_stream_options_for_reasoning_model` 通过。
- `before_provider_request_hook_patches_actual_provider_request` 通过。

## 阶段 2：拆出 tool execution runtime

### 目的

把 sequential/parallel tool execution 从 `agent_loop.rs` 中拆出，并统一 before/after hook、unknown tool、blocked tool、result normalization、message append 的规则。

### 文件范围

- Create: `crates/pi-agent-core/src/loop_runtime/tools.rs`
- Modify: `crates/pi-agent-core/src/loop_runtime/mod.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Test: `crates/pi-agent-core/tests/agent_loop.rs`
- Test: `crates/pi-agent-core/tests/hooks.rs`
- Test: `crates/pi-agent-core/tests/parallel_tools.rs`

### 设计

新增内部类型：

```rust
pub(crate) struct ToolCallRequest {
    pub index: usize,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

pub(crate) struct ToolCallExecution {
    pub index: usize,
    pub tool_call_id: String,
    pub tool_name: String,
    pub result: crate::types::AgentToolResult,
}

pub(crate) enum ToolRuntimeEvent {
    Start {
        tool_call_id: String,
        tool_name: String,
    },
    Update {
        tool_call_id: String,
        tool_name: String,
        update: crate::types::AgentToolOutput,
    },
    End {
        tool_call_id: String,
        tool_name: String,
        result: crate::types::AgentToolResult,
    },
}
```

新增函数：

```rust
pub(crate) fn extract_tool_calls(
    assistant: &pi_ai::types::AssistantMessage,
) -> Vec<ToolCallRequest>
```

```rust
pub(crate) fn should_use_sequential_tools(
    global_mode: crate::types::ToolExecutionMode,
    calls: &[ToolCallRequest],
    tools: &[crate::types::AgentTool],
) -> bool
```

此阶段不必一次性把 async stream event 产出完全抽象掉。建议第一步只抽出纯函数和结果写入 helper：

```rust
pub(crate) fn append_tool_result_messages(
    messages: &mut Vec<crate::types::AgentMessage>,
    executions: &[ToolCallExecution],
)
```

第二步再把 sequential 和 parallel executor 移入 `tools.rs`，保留通过 callback emit event 的形式：

```rust
pub(crate) async fn execute_tool_batch<E, Fut>(
    state: std::sync::Arc<std::sync::RwLock<crate::agent::AgentState>>,
    assistant: pi_ai::types::AssistantMessage,
    calls: Vec<ToolCallRequest>,
    emit: E,
) -> Result<Vec<ToolCallExecution>, String>
where
    E: Fn(ToolRuntimeEvent) -> Fut,
    Fut: std::future::Future<Output = ()>;
```

如果 callback lifetime 让实现复杂，可以先只移动 pure helpers 和 parallel preparation，保留 event-yield loop 在 `agent_loop.rs`。不要为了“一步到位”引入难懂的 stream abstraction。

### 步骤

- [ ] 添加 `extract_tool_calls` 单元测试，覆盖多个 `ContentBlock::ToolCall` 保持 assistant 顺序。
- [ ] 添加 `should_use_sequential_tools` 单元测试，覆盖 global sequential、global parallel、per-tool sequential override。
- [ ] 添加 `append_tool_result_messages` 单元测试，断言 `message_id == tool_call_id`、`tool_name`、`is_error`、`content` 保持当前行为。
- [ ] 在 `agent_loop.rs` 中用 `extract_tool_calls` 替代内联 filter_map。
- [ ] 在 `agent_loop.rs` 中用 `should_use_sequential_tools` 替代内联 `has_sequential_override`。
- [ ] 在 sequential 和 parallel 分支都调用 `append_tool_result_messages`，减少重复 push 逻辑。
- [ ] 第二轮再移动 sequential executor，保持 `ToolCallUpdate` 只在 sequential path 支持的当前行为。
- [ ] 第三轮移动 parallel executor，保持 parallel `ToolCallEnd` completion order 和 message append assistant order 的当前行为。

### 验收标准

```bash
cargo test -p pi-agent-core --test agent_loop
cargo test -p pi-agent-core --test hooks
cargo test -p pi-agent-core --test parallel_tools
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- `tool_use_turn_executes_tool` 通过。
- `unknown_tool_yields_error_content_and_continues` 通过。
- `tool_update_events_stream_before_tool_end` 通过。
- `before_hook_blocks_tool_execution` 通过。
- `after_hook_replaces_tool_result` 通过。
- `after_hook_terminate_stops_loop_after_tool_results` 通过。
- `parallel_tool_results_are_appended_in_assistant_order` 通过。
- `parallel_tool_end_events_are_emitted_in_completion_order` 通过。

## 阶段 3：收敛 `AgentState` 写入口

### 目的

把状态修改从“外部直接写字段”改为“通过状态方法表达意图”，减少 lock 使用重复和 message id 生成问题。

### 文件范围

- Modify: `crates/pi-agent-core/src/agent.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Modify: `crates/pi-agent-core/src/loop_runtime/context.rs`
- Modify: `crates/pi-agent-core/src/loop_runtime/tools.rs`
- Test: `crates/pi-agent-core/tests/queues_thinking.rs`
- Test: `crates/pi-agent-core/tests/agent_loop.rs`

### 设计

给 `AgentState` 增加内部计数器：

```rust
pub struct AgentState {
    pub messages: Vec<AgentMessage>,
    pub tools: Vec<AgentTool>,
    pub config: AgentConfig,
    pub cancel_token: CancellationToken,
    pub steering_queue: VecDeque<AgentMessage>,
    pub follow_up_queue: VecDeque<AgentMessage>,
    pub(crate) provider_request_override: Option<ProviderRequestOverride>,
    next_message_seq: u64,
}
```

新增方法：

```rust
impl AgentState {
    fn next_message_id(&mut self, prefix: &str) -> String {
        let id = format!("{}_{}", prefix, self.next_message_seq);
        self.next_message_seq += 1;
        id
    }

    fn push_user_text(&mut self, prefix: &str, text: String) {
        let message_id = self.next_message_id(prefix);
        self.messages.push(AgentMessage::UserText { message_id, text });
    }

    fn enqueue_steering(&mut self, text: String) {
        let message_id = self.next_message_id("steer");
        self.steering_queue
            .push_back(AgentMessage::UserText { message_id, text });
    }

    fn enqueue_follow_up(&mut self, text: String) {
        let message_id = self.next_message_id("followup");
        self.follow_up_queue
            .push_back(AgentMessage::UserText { message_id, text });
    }
}
```

注意：这会改变现有 id 行为。必须在阶段 0 的 characterization test 基础上明确更新预期。

### 步骤

- [ ] 在 `AgentState::new` 等价初始化路径中设置 `next_message_seq: 0`。
- [ ] 把 `Agent::steer` 的 read lock + write lock 两步改成单个 write lock 调用 `enqueue_steering`。
- [ ] 把 `Agent::follow_up` 改成单个 write lock 调用 `enqueue_follow_up`。
- [ ] 把 `prompt_internal` 中 `user_{messages.len()}` 改为 `push_user_text("user", text)`。
- [ ] 把 `AgentHarness::prompt` 中预插入的 user id 生成保持当前策略，或者改为委托 Agent 统一生成。建议本阶段保持 harness 行为不变，避免同时改变 prompt lifecycle。
- [ ] 更新 `queues_thinking` 中阶段 0 新增的 characterization test，断言 drain 后 id 不重复。
- [ ] 检查 session tests 是否依赖具体 `user_0` id。如果依赖，保留 `AgentHarness` 的外部 prompt id 行为，不影响 session wire。

### 验收标准

```bash
cargo test -p pi-agent-core --test queues_thinking
cargo test -p pi-agent-core --test agent_hydration
cargo test -p pi-agent-core --test agent_loop
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- `steer` / `follow_up` drain 后不会重复生成同一 id。
- 不改变 LLM-facing message content。
- 不改变 session wire shape。

## 阶段 4：拆分 `types.rs`

### 目的

把 public domain types 拆成多个文件，降低单文件上下文和 API 审查成本，同时通过 re-export 保持调用方兼容。

### 文件范围

- Create: `crates/pi-agent-core/src/types/mod.rs`
- Create: `crates/pi-agent-core/src/types/config.rs`
- Create: `crates/pi-agent-core/src/types/event.rs`
- Create: `crates/pi-agent-core/src/types/message.rs`
- Create: `crates/pi-agent-core/src/types/resource.rs`
- Create: `crates/pi-agent-core/src/types/tool.rs`
- Delete after migration: `crates/pi-agent-core/src/types.rs`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Test: all crate tests

### 拆分规则

`types/message.rs`：

- `AgentMessage`

`types/tool.rs`：

- `AgentToolOutput`
- `AgentToolResult`
- `ToolFn`
- `ToolUpdateCallback`
- `AgentTool`
- `ToolExecutionMode`

`types/resource.rs`：

- `Skill`
- `PromptTemplate`
- `AgentResources`
- `ResourceDiagnostic`
- `DiagnosticSeverity`
- `SourceTag`
- `SourcedSkill`
- `SourcedPromptTemplate`
- `SourcedResourceDiagnostic`

`types/config.rs`：

- `ThinkingLevel`
- `QueueMode`
- `CompactionSettings`
- `CompactionConfig`
- `AgentConfig`

`types/event.rs`：

- `ProviderRequestSnapshot`
- `AgentEvent`
- `AgentStream`

`types/mod.rs`：

```rust
pub mod config;
pub mod event;
pub mod message;
pub mod resource;
pub mod tool;

pub use config::{AgentConfig, CompactionConfig, CompactionSettings, QueueMode, ThinkingLevel};
pub use event::{AgentEvent, AgentStream, ProviderRequestSnapshot};
pub use message::AgentMessage;
pub use resource::{
    AgentResources, DiagnosticSeverity, PromptTemplate, ResourceDiagnostic, Skill, SourceTag,
    SourcedPromptTemplate, SourcedResourceDiagnostic, SourcedSkill,
};
pub use tool::{
    AgentTool, AgentToolOutput, AgentToolResult, ToolExecutionMode, ToolFn, ToolUpdateCallback,
};
```

### 步骤

- [ ] 创建 `src/types/` 目录和上述文件。
- [ ] 移动类型定义，不改变字段名、derive、constructor、`FromStr`、`Display` 实现。
- [ ] 把原 `types.rs` 底部 tests 按领域移动到对应文件。
- [ ] 在 `lib.rs` 中保持 `pub mod types;`。
- [ ] 保持 root `pub use types::{...};` 不变。
- [ ] 运行 `cargo fmt` 修正 import 排序。

### 验收标准

```bash
cargo test -p pi-agent-core
cargo fmt --check
cargo check -p pi-agent-core
```

期望：

- 所有外部测试无需改 import。
- `pi_agent_core::types::AgentMessage` 路径可用。
- `pi_agent_core::AgentMessage` root re-export 可用。

## 阶段 5：拆分 `harness.rs`

### 目的

把 harness 的类型、subscription、provider hook、patch 逻辑拆开，降低后续新增 plugin/session lifecycle 能力时的冲突。

### 文件范围

- Create: `crates/pi-agent-core/src/harness/mod.rs`
- Create: `crates/pi-agent-core/src/harness/events.rs`
- Create: `crates/pi-agent-core/src/harness/hooks.rs`
- Create: `crates/pi-agent-core/src/harness/patch.rs`
- Create: `crates/pi-agent-core/src/harness/phase.rs`
- Create: `crates/pi-agent-core/src/harness/provider.rs`
- Create: `crates/pi-agent-core/src/harness/subscription.rs`
- Delete after migration: `crates/pi-agent-core/src/harness.rs`
- Modify: `crates/pi-agent-core/src/lib.rs`
- Test: harness tests

### 拆分规则

`harness/events.rs`：

- `AgentHarnessEvent`
- `map_agent_event`

`harness/hooks.rs`：

- `HarnessContext`
- `AgentHarnessHooks`
- hook type aliases
- `HarnessHookFuture`
- `HarnessHookKind`
- `on_kind`
- `OnHandlerRegistry`
- `OnHandlerEntry`

`harness/patch.rs`：

- `Patch<T>`
- `HeaderPatch`
- `StreamOptionsPatch`
- `apply_stream_options_patch`
- `apply_header_patch`
- `merge_headers`

`harness/phase.rs`：

- `AgentHarnessPhase`
- `PhaseResetOnDrop`

`harness/provider.rs`：

- `BeforeProviderRequest`
- `BeforeProviderRequestPatch`
- `ProviderAuth`
- `BeforeProviderPayload`
- `BeforeProviderPayloadPatch`
- `ProviderResponse`
- `make_provider_request_hook`
- `make_stream_hooks`

`harness/subscription.rs`：

- `Observer`
- `ObserverEntry`
- `SubscriptionGuard`

`harness/mod.rs`：

- `AgentHarness`
- `AbortResult`
- public re-exports from submodules

### 步骤

- [ ] 先移动 pure patch functions 到 `harness/patch.rs`，运行 `cargo test -p pi-agent-core --test m9_harness`。
- [ ] 再移动 events 和 `map_agent_event` 到 `harness/events.rs`。
- [ ] 再移动 subscription types 到 `harness/subscription.rs`。
- [ ] 再移动 provider hook composition 到 `harness/provider.rs`。
- [ ] 最后移动 hook registry 和 phase types。
- [ ] 每移动一个子模块后运行对应 harness tests，避免一次性大搬迁。

### 验收标准

```bash
cargo test -p pi-agent-core --test m9_harness
cargo test -p pi-agent-core --test harness_subscribe
cargo test -p pi-agent-core --test harness_types
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- `pi_agent_core::harness::AgentHarness` 路径可用。
- root re-export 的 `AgentHarness`、`AgentHarnessEvent`、`Patch`、`StreamOptionsPatch`、`HeaderPatch`、`ProviderAuth` 等仍可用。
- `subscribe_guard_drop_removes_listener` 通过。
- `provider_request_auth_and_patch_merge_delete_apply_to_each_provider_call` 通过。

## 阶段 6：引入结构化 `AgentLoopError`

### 目的

逐步替换 loop 内部的 `String` 错误，为 TUI、harness、session diagnostics 提供稳定错误分类。

### 文件范围

- Modify: `crates/pi-agent-core/src/errors.rs`
- Create: `crates/pi-agent-core/src/loop_runtime/error.rs`
- Modify: `crates/pi-agent-core/src/agent_loop.rs`
- Modify: `crates/pi-agent-core/src/hooks.rs`
- Modify: `crates/pi-agent-core/src/types/event.rs` if 阶段 4 已完成，否则 `crates/pi-agent-core/src/types.rs`
- Test: `crates/pi-agent-core/tests/agent_loop.rs`
- Test: `crates/pi-agent-core/tests/hooks.rs`
- Test: `crates/pi-agent-core/tests/compaction.rs`

### 设计

第一轮只引入内部错误，不改变 public event：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentLoopErrorCode {
    Aborted,
    MaxTurnsExceeded,
    InvalidState,
    Provider,
    ProviderStream,
    Hook,
    Tool,
    Compaction,
    Unknown,
}

impl AgentLoopErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentLoopErrorCode::Aborted => "aborted",
            AgentLoopErrorCode::MaxTurnsExceeded => "max_turns_exceeded",
            AgentLoopErrorCode::InvalidState => "invalid_state",
            AgentLoopErrorCode::Provider => "provider",
            AgentLoopErrorCode::ProviderStream => "provider_stream",
            AgentLoopErrorCode::Hook => "hook",
            AgentLoopErrorCode::Tool => "tool",
            AgentLoopErrorCode::Compaction => "compaction",
            AgentLoopErrorCode::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct AgentLoopError {
    pub code: AgentLoopErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl AgentLoopError {
    pub fn new(code: AgentLoopErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}
```

第二轮扩展 `AgentEvent::AgentError`：

```rust
AgentError {
    error: String,
    code: Option<String>,
    details: Option<serde_json::Value>,
}
```

为了减少破坏，建议先新增 event variant：

```rust
AgentErrorDetailed {
    error: String,
    code: String,
    details: Option<serde_json::Value>,
}
```

但这会影响消费者匹配逻辑。更保守的做法是保持 `AgentError { error }`，把 typed error 用在内部和 harness mapping，等下游准备好再扩展 event。

推荐本阶段采用保守做法：内部 typed error，public event 不变。

### 步骤

- [ ] 添加 `AgentLoopErrorCode` 和 `AgentLoopError`。
- [ ] 给 `AgentLoopError` 实现 `From<String>`，code 使用 `Unknown`。
- [ ] 把 `compact_before_provider_request` 返回类型从 `Result<_, String>` 改为 `Result<_, AgentLoopError>`。
- [ ] 把 `prepare_next_turn` 返回类型改为 `Result<(), AgentLoopError>`，hook error code 使用 `Hook`。
- [ ] 把 provider stream ended without Done 的错误 code 设为 `ProviderStream`。
- [ ] 把 max turns exceeded 的错误 code 设为 `MaxTurnsExceeded`。
- [ ] 在 yield `AgentEvent::AgentError` 时继续只输出 `err.message`，保持 public behavior。
- [ ] 后续单独开阶段修改 public event payload。

### 验收标准

```bash
cargo test -p pi-agent-core --test agent_loop
cargo test -p pi-agent-core --test hooks
cargo test -p pi-agent-core --test compaction
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- 外部测试不用改 event pattern。
- 内部错误 code 有单元测试覆盖 `as_str()`。
- `max_turns_exceeded_yields_error` 仍看到原有 error 文本。
- `runtime_compaction_summarizes_before_provider_request` 仍通过。

## 阶段 7：清理 harness 迁移中间态

### 目的

移除或明确标注当前不可达/无效代码，避免后续开发误用。

### 文件范围

- Modify: `crates/pi-agent-core/src/harness/phase.rs` if 阶段 5 已完成，否则 `crates/pi-agent-core/src/harness.rs`
- Modify: `crates/pi-agent-core/src/harness/mod.rs` if 阶段 5 已完成，否则 `crates/pi-agent-core/src/harness.rs`
- Test: `crates/pi-agent-core/tests/harness_subscribe.rs`
- Test: `crates/pi-agent-core/tests/m9_harness.rs`

### 具体项

#### `AgentHarnessPhase`

当前：

```rust
pub enum AgentHarnessPhase {
    Idle,
    Turn,
    Compaction,
    BranchSummary,
}
```

建议保留 enum，但把不可达状态文档改得更明确：

```rust
/// `Compaction` and `BranchSummary` are compatibility placeholders for the TS
/// phase space. Current Rust `AgentHarness` does not enter them; callers should
/// not rely on observing these variants until explicit compact/tree APIs are
/// added.
```

如果要更严格，可以把不可达 variants 标为 internal，不建议本阶段这么做，因为这会破坏 public API。

#### `AgentHarness::prompt` 中无效 `config` 逻辑

当前逻辑等价于：

```rust
let config = None;
```

建议直接改成：

```rust
let system_prompt = None;
```

并在 `HarnessContext` 构造处使用：

```rust
let mut harness_context = HarnessContext {
    messages,
    system_prompt,
};
```

如果后续要从 `SystemPrompt` message 提取 system prompt，应单独设计并加测试，不在这个清理阶段混入。

### 验收标准

```bash
cargo test -p pi-agent-core --test harness_subscribe
cargo test -p pi-agent-core --test m9_harness
cargo test -p pi-agent-core
cargo fmt --check
```

期望：

- `phase_starts_idle_and_returns_to_idle_after_prompt` 通过。
- `phase_is_turn_during_active_prompt` 通过。
- harness prompt 行为无变化。

## 阶段 8：评估并收紧 public API

### 目的

在完成结构拆分后，审查 root exports 和 public modules，减少无意暴露的内部实现。

### 文件范围

- Modify: `crates/pi-agent-core/src/lib.rs`
- Review: `crates/pi-agent-core/src/types/mod.rs`
- Review: `crates/pi-agent-core/src/harness/mod.rs`
- Review: `crates/pi-agent-core/src/loop_runtime/*`

### 当前 root exports

`lib.rs` 当前导出大量类型：

```rust
pub mod agent;
pub mod agent_loop;
pub mod branch_summary;
pub mod compaction;
pub mod convert;
pub mod env;
pub mod errors;
pub mod harness;
pub mod hooks;
pub mod proxy;
pub mod queues;
pub mod resources;
pub mod session;
pub mod shell_output;
pub mod truncate;
pub mod types;
```

建议：

- 保持 `agent`、`harness`、`session`、`resources`、`types` 公开。
- `agent_loop` 暂时保持公开，避免破坏测试和下游，但标注为 lower-level API。
- 新增 `loop_runtime` 必须保持 `mod loop_runtime;`，不要公开。
- `queues` 如果只服务内部，可以在后续 major cleanup 中改为 private；当前先不动。

### 验收标准

```bash
cargo check --workspace
cargo test -p pi-agent-core
cargo test --workspace
cargo fmt --check
```

期望：

- 下游 crates 编译通过。
- 没有新增 public `loop_runtime` API。
- root exports 兼容当前调用方。

## 风险与缓解

### 风险 1：拆文件时行为顺序变化

高风险区域：

- `should_stop_after_turn` 在 stop/tool_use 后的调用位置。
- `prepare_next_turn` 与 follow-up queue drain 的相对顺序。
- parallel tool `ToolCallEnd` completion order 和 message append assistant order。
- provider request hook 必须每次 provider call 都执行。

缓解：

- 每个拆分任务只移动一个职责。
- 每次移动后运行对应 integration test。
- 不在同一 commit 中同时移动代码和改行为。

### 风险 2：message id 行为改变影响 session/tree

阶段 3 会改变 steer/follow-up id 生成。

缓解：

- 阶段 0 先写 characterization test。
- 阶段 3 单独提交，明确 commit message。
- 检查所有 session tests 是否依赖具体 id。

### 风险 3：typed error 引入后 public event 破坏调用方

缓解：

- 第一轮只在内部使用 `AgentLoopError`。
- `AgentEvent::AgentError { error }` 暂时不变。
- 等 TUI/harness 对 detailed error 有需求时，再做 event schema migration。

### 风险 4：过度抽象 tool executor

如果把 executor 过早做成复杂 stream combinator，会比当前代码更难读。

缓解：

- 先抽 pure helpers。
- 再抽 append result helper。
- 最后才移动 async execution。
- 保持函数签名朴素，避免 generic stream 类型泄露到 public API。

## 推荐提交顺序

1. `test: add pi-agent-core refactor characterization coverage`
2. `refactor(agent-core): extract provider request preparation`
3. `refactor(agent-core): extract tool call helpers`
4. `refactor(agent-core): consolidate agent state message ids`
5. `refactor(agent-core): split public types by domain`
6. `refactor(agent-core): split harness modules`
7. `refactor(agent-core): add internal agent loop error type`
8. `chore(agent-core): clean harness migration leftovers`
9. `docs: document pi-agent-core architecture boundaries`

每个 commit 后至少运行：

```bash
cargo test -p pi-agent-core
cargo fmt --check
```

涉及 public API 或 workspace import 时运行：

```bash
cargo check --workspace
cargo test --workspace
```

## 最终验收标准

完成全部阶段后，应该满足：

- `agent_loop.rs` 明显缩短，主循环只表达 lifecycle，不再内联 request preparation 和大段 tool execution。
- `harness.rs` 被拆分为 `harness/` 目录，provider hook/patch/subscription/event 类型各自有边界。
- `types.rs` 被拆分为 `types/` 目录，root re-export 保持兼容。
- `AgentState` 的主要写入口通过方法表达，不再由多个模块随意拼装 message id。
- loop 内部有结构化错误 code，即使 public event 仍保持 string。
- `cargo test -p pi-agent-core` 通过。
- `cargo fmt --check` 通过。
- `cargo check --workspace` 通过。
- 测试 warning 被清理或限制在明确的 test helper allow 中。

## 后续可选优化

这些不建议放进第一轮结构优化：

- 把 `std::sync::RwLock` 改成 `tokio::sync::RwLock` 或 actor-style state owner。
- 给 `AgentEvent::AgentError` 增加 code/details。
- 给 parallel tool path 增加 `ToolCallUpdate` 支持。
- 把 hook return type 从 `String` 错误迁移到 typed hook error。
- 引入 `AgentBuilder`，收敛 `AgentConfig` 的构造和默认值。
- 把 session storage 与 harness prompt lifecycle 更紧密地集成。
- 对 `AgentMessage` 做 serde 直接支持，减少 session wire 和 runtime message 的转换重复。

这些优化依赖第一轮边界清理。等本方案的阶段 1 到阶段 7 完成后，再单独评估收益和破坏面。
