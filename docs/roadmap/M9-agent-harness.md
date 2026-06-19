# M9 — agent-core harness 完备

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：无（不依赖认证）· 解锁：[M11](M11-interactive-ux.md)、[M12](M12-plugin-system.md)
> 定位：**核心引擎补全**。`pi-agent-core` 运行时内核已完整（见 [done.md](done.md) 修正项），
> 本里程碑补齐其上层 **harness 包装**与**抽象层**。

## 目标
把 `pi-agent-core` 从"底层 `Agent` + `Session` 分离"推进到对标 pi 的 `AgentHarness` 完整编排层。

## 待实现项

### 1. AgentHarness 包装层
- ✅ 基础高层包装：`AgentHarness` 已接入 `Agent`，支持 `prompt()`、`messages()`、`abort()`，并通过 `Agent::run()` 复用底层 loop。
- ✅ harness 事件骨架：已覆盖并导出 `before_agent_start` / `context` / `before_provider_request` / `tool_call` / `tool_result` / `session_compact` / `settled` 等事件枚举；底层 `AgentEvent` 通过 `AgentHarnessEvent::Agent` 透传。
- ✅ harness 钩子基础：`before_agent_start` / `context` / `before_provider_request` hook 已可 patch context/request；测试覆盖启动消息 patching。
- ✅ provider request patching：`before_provider_request` hook 返回的 `Context` / `StreamOptions` 已通过一次性 override 进入实际 provider call；事件 payload 使用真实 agent state 的 messages/tools/resources/stream options snapshot。
- ⚠️ 仍需继续对齐 TS `agent-harness.ts` 的 session 持久化 orchestration、事件 payload 细节、provider payload/response hooks、stream option patch merge/delete 语义与更完整的结果 patching。

### 2. 自定义消息类型
- ✅ `AgentMessage` 新增 `BashExecution`（command/output/exitCode/cancelled/truncated）、`Custom`（customType/content/display/details）、`BranchSummary`。
- ✅ `convert.rs` 已处理三类消息：bash execution 格式化为 TS 对应文本，custom content 进入 user message，branch summary 使用 TS prefix/suffix 语义。
- ✅ session wire 已支持 `role: "bashExecution" | "custom" | "branchSummary"` 的序列化/读回。
- Rust 设计：用 enum 新增 variant（不照搬 TS 的 declaration merging；扩展性交给 [M12](M12-plugin-system.md) 的 trait/Lua）。

### 3. FileSystem / Shell / ExecutionEnv 抽象
- ✅ `FileSystem` trait 已覆盖 `read_text_file` / `write_file` / `append_file` / `list_dir` / `create_dir` / `remove` / `canonical_path` / temp file/dir 等核心方法。
- ✅ `Shell` trait 已覆盖 `exec(command, options) -> ExecutionOutput { stdout, stderr, exit_code }`。
- ✅ `ExecutionEnv = FileSystem + Shell + cleanup()` 已落地。
- ✅ 提供 `InMemoryExecutionEnv`，用于离线测试和后续工具沙箱注入。
- 价值：把工具与执行环境解耦，便于测试注入与沙箱（也是 [M12](M12-plugin-system.md) 插件安全的基础）。

### 4. 类型化错误
- ✅ `FileError`/`ExecutionError`/`AgentHarnessError`/`BranchSummaryError` 已落地。
- ✅ 每类错误暴露稳定 code enum 与 TS code 字符串语义对应。
- Rust 设计：`thiserror` enum + code，保持与 TS code 字符串语义一致。

### 5. 其它 harness 能力
- ⚠️ **proxy 流式**：`streamProxy()` 尚未实现。
- ⚠️ **branch summarization**：消息类型和错误类型已准备，真实会话树分支摘要尚未实现。
- ✅ `transformContext` 的核心等价能力由 `before_agent_start` / `context` hook patch messages 覆盖第一步。
- ⚠️ `getApiKey` 动态解析 hook 尚未接入 provider stream options。
- ⚠️ 输出截断工具、shell 输出格式化尚未从 `pi-coding-agent` 工具侧上移到 harness 公共层。

## 验收 / 测试（离线优先）
- ✅ harness 事件/钩子：`tests/m9_harness.rs` 用 faux provider 跑一轮，断言事件序列与 patching 生效。
- ✅ 抽象层：提供内存 `FileSystem` + faux `Shell` 实现供测试。
- ✅ 自定义消息：session wire 序列化 + convert 到 LLM context 的断言。

## 本轮落地
- 新增 `errors.rs`、`env.rs`、`harness.rs`。
- 扩展 `AgentMessage`、`convert_to_context`、compaction 估算/摘要输入、session JSONL wire、session context 读回。
- 新增 `Agent::run()`，让 harness 能在运行前 patch messages 而不重复插入 prompt。
- 新增 `Agent::provider_request_snapshot()` 与 provider request override，让 harness 的 `before_provider_request` patch 真正影响下一次 provider 调用。
- 已验证：`cargo test -p pi-agent-core` 通过。
