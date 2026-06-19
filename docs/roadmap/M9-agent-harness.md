# M9 — agent-core harness 完备

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：无（不依赖认证）· 解锁：[M11](M11-interactive-ux.md)、[M12](M12-plugin-system.md)
> 定位：**核心引擎补全**。`pi-agent-core` 运行时内核已完整（见 [done.md](done.md) 修正项），
> 本里程碑补齐其上层 **harness 包装**与**抽象层**。

## 目标
把 `pi-agent-core` 从"底层 `Agent` + `Session` 分离"推进到对标 pi 的 `AgentHarness` 完整编排层。

## 待实现项

### 1. AgentHarness 包装层
- 高层包装类，编排 `Agent` + `Session` + hooks + 事件。TS：`agent/src/harness/agent-harness.ts`（~1064 行）。
- **harness 事件系统**：20+ 事件（`before_agent_start`/`context`/`before_provider_request`/`tool_call`/`tool_result`/`session_compact`/`session_before_tree`…）。TS：`harness/types.ts:634-660`。
- **harness 钩子系统**：上述事件的钩子 + 结果 patching。TS：`harness/types.ts:704-724`。

### 2. 自定义消息类型
- `BashExecutionMessage`（command/output/exitCode/cancelled/truncated）、`CustomMessage`（customType/content/display/details）、`BranchSummaryMessage`。TS：`harness/messages.ts:19-45`。
- 在 `convert.rs` 的 `convert_to_llm` / `convert_to_context` 中处理这些类型（当前仅处理 UserText/Assistant/ToolResult/CompactionSummary）。
- Rust 设计：用 enum 新增 variant（不照搬 TS 的 declaration merging；扩展性交给 [M12](M12-plugin-system.md) 的 trait/Lua）。

### 3. FileSystem / Shell / ExecutionEnv 抽象
- `FileSystem` trait（~18 方法：readTextFile/writeFile/listDir/createDir/remove/canonicalPath…）。
- `Shell` trait（`exec(command, options) -> {stdout, stderr, exitCode}`）。
- `ExecutionEnv = FileSystem + Shell + cleanup()`。TS：`harness/types.ts:268-332`。
- 价值：把工具与执行环境解耦，便于测试注入与沙箱（也是 [M12](M12-plugin-system.md) 插件安全的基础）。

### 4. 类型化错误
- `FileError`/`ExecutionError`/`AgentHarnessError`/`BranchSummaryError`（含 code 字段）。当前仅 `SessionError` 有。TS：`harness/types.ts:122-227`。
- Rust 设计：`thiserror` enum + code，保持与 TS code 字符串语义一致。

### 5. 其它 harness 能力
- **proxy 流式**：`streamProxy()` 从 server delta 事件重建部分消息。TS：`agent/src/proxy.ts`。
- **branch summarization**：会话树分支摘要。TS：`harness/compaction/branch-summarization.ts`（263 行）。
- `transformContext` 钩子（转换前 AgentMessage[] 操作）、`getApiKey` 动态解析钩子（mid-run 解析 OAuth token）。
- 输出截断工具（`harness/utils/truncate.ts` 344 行）、shell 输出格式化（`shell-output.ts` 143 行）。

## 验收 / 测试（离线优先）
- harness 事件/钩子：用 faux provider 跑一轮，断言事件序列与 patching 生效。
- 抽象层：提供内存 `FileSystem` + faux `Shell` 实现供测试。
- 自定义消息：往返序列化 + convert 到 LLM context 的断言。
