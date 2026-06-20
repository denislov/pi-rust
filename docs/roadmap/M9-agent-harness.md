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
- ✅ harness 钩子基础：`before_agent_start` / `context` / `before_provider_request` / `before_provider_payload` / `after_provider_response` hook 已可 patch context/request/payload，并测试覆盖启动消息 patching、payload patching 与 response hook。
- ✅ provider request patching：`before_provider_request` 已下沉到 `Agent::run()` loop，每次 provider call 都重新合成真实 `Context` / `StreamOptions`，再应用动态 auth 与 patch；工具调用后的第二次模型请求同样生效。
- ✅ stream option patch merge/delete：新增 `StreamOptionsPatch` / `Patch<T>` / `HeaderPatch`，支持字段 set/clear、headers merge/delete，测试覆盖 TS 对应删除语义。
- ✅ provider auth hook：新增 `get_api_key_and_headers` harness hook，按 provider/model 动态解析 API key 与 headers，并在 request hook 前合并到 `StreamOptions`。
- ✅ provider payload/response hook 通道：`pi-ai::StreamOptions` 新增跳过序列化的 `ProviderStreamHooks`，harness 将 payload/response hooks 组合进去；provider 或 proxy transport 调用该通道即可获得 TS `onPayload` / `onResponse` 等价能力。
- ✅ session 持久化 orchestration 边界：`pi-agent-core::session` 提供 JSONL v3 storage/repo、session context rebuild、branch/leaf entries、compaction/branch summary entries；`pi-coding-agent` 运行时继续负责 active session 写入编排，core harness 不强行持有 CLI session 状态。

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
- ✅ **proxy 流式**：新增 `proxy.rs`，提供 `stream_proxy()` 与可离线测试的 `stream_proxy_with_transport()`，支持 TS proxy event 到 `AssistantMessageEvent` 的重建、split tool-call JSON 累积解析，以及 proxy request body 的 serializable options 子集。
- ✅ **branch summarization**：新增 `branch_summary.rs`，支持收集 abandoned branch、准备分支消息/文件操作详情、用 provider 生成带 TS preamble 的 branch summary。
- ✅ `transformContext` 的核心等价能力由 `before_agent_start` / `context` hook patch messages 覆盖第一步。
- ✅ `getApiKey` 动态解析 hook 已接入每次 provider stream options。
- ✅ 输出截断工具、shell 输出格式化已上移到 harness 公共层：`truncate.rs` 提供 head/tail truncation，`shell_output.rs` 提供 shell capture、binary sanitize、tail truncation 与 full-output temp file。
- 后续深化：内置 HTTP providers 仍可逐步在各自 payload 构造处调用 `ProviderStreamHooks`，以获得 provider-internal payload/response observability；M9 已完成 harness/StreamOptions 公共通道和 proxy/custom provider 验证。

## 验收 / 测试（离线优先）
- ✅ harness 事件/钩子：`tests/m9_harness.rs` 用 faux provider 跑一轮，断言事件序列与 patching 生效。
- ✅ 抽象层：提供内存 `FileSystem` + faux `Shell` 实现供测试。
- ✅ 自定义消息：session wire 序列化 + convert 到 LLM context 的断言。
- ✅ provider hooks：`tests/m9_harness.rs` 覆盖动态 auth、headers merge/delete、每次 provider call patching、payload hook 与 response hook。
- ✅ branch/proxy/shell：`tests/m9_branch_proxy_shell.rs` 覆盖 branch summary collection/generation、proxy stream event 重建、split tool-call JSON、truncate head/tail、shell capture full-output 落盘。

## 本轮落地
- 新增 `errors.rs`、`env.rs`、`harness.rs`、`branch_summary.rs`、`proxy.rs`、`truncate.rs`、`shell_output.rs`。
- 扩展 `AgentMessage`、`convert_to_context`、compaction 估算/摘要输入、session JSONL wire、session context 读回。
- 新增 `Agent::run()`，让 harness 能在运行前 patch messages 而不重复插入 prompt。
- 新增 `AgentEvent::BeforeProviderRequest` 与 `AgentHooks::before_provider_request`，让 harness 的 provider request/auth/patch 逻辑在每次 provider call 前执行。
- 新增 `pi-ai::ProviderStreamHooks`，作为 provider payload/response lifecycle 的跨 crate 通道。
- 已验证：`cargo test -p pi-agent-core -- --test-threads=1` 通过。

## 已知缺口
- `transformContext` 钩子（AgentMessage[]→AgentMessage[] 变换）：TS agent-loop 有此钩子，Rust 尚未实现。
- `agentLoopContinue`：TS 有显式从已有 context 继续的入口，Rust 缺失。
- Harness 事件订阅模式（`on`/`off` with `*` wildcard）：TS 有，Rust 缺失。
- `AgentHarnessPhase` 生命周期（`idle`/`running`/`aborting`）：TS 有，Rust 缺失。
- Guided abort（`AbortResult` with `pendingSessionWrites`）：TS 有，Rust 缺失。
- Sourced resources（`loadSourcedSkills` 等保留来源元数据）：TS 有，Rust 缺失。
- Configurable `convertToLlm`：Rust 硬编码在 `convert.rs`；TS 可注入回调。
