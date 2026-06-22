该仓库采用 Rust 生态标准的 `thiserror` 库进行错误定义，并结合领域驱动设计（DDD）思想，在不同模块中实现了细粒度的错误类型。整体架构遵循“结构化错误码 + 描述性消息”的模式，并在异步流（Stream）和 RPC 协议层建立了统一的错误传播通道。

### 1. 核心系统与模式
*   **错误定义框架**：全面使用 `thiserror::Error` 派生宏，确保错误类型具备标准的 `Display` 和 `Debug` 实现，并支持自动化的错误源链（Error Chain）。
*   **错误码枚举（ErrorCode）**：在核心业务逻辑中，错误通常由一个 `enum ErrorCode` 和一个包含消息的结构体组成。这种设计便于前端或上层调用者通过 `code.as_str()` 进行精确的逻辑分支判断（如重试、提示用户等），而不仅仅是依赖字符串匹配。
*   **流式错误传播**：在 AI 交互循环（Agent Loop）中，错误被封装为 `AssistantMessageEvent::Error` 或 `AgentEvent::AgentError`，通过 `futures::Stream` 向下游传递，确保长连接中的异常不会导致进程崩溃，而是作为事件被消费。

### 2. 关键文件与模块
*   **`crates/pi-agent-core/src/errors.rs`**：定义了核心领域的通用错误，包括 `FileError`（文件系统）、`ExecutionError`（命令执行）和 `AgentHarnessError`（智能体调度）。每个错误都配有对应的 `ErrorCode` 枚举。
*   **`crates/pi-agent-core/src/session/error.rs`**：定义了会话管理相关的 `SessionError`，涵盖 NotFound、InvalidSession 等状态。
*   **`crates/pi-coding-agent/src/error.rs`**：定义了 CLI 层的 `CliError`，用于处理参数解析、模型选择失败等用户输入层面的错误。
*   **`crates/pi-tui/src/tui.rs`**：定义了界面渲染层的 `TuiError`，主要处理终端 I/O 异常和行宽校验失败。
*   **`crates/pi-ai/src/stream.rs`**：提供了 `complete` 辅助函数，将流式的 `EventStream` 收敛为 `Result<AssistantMessage, String>`，简化了非流式调用的错误处理。

### 3. 架构约定与设计决策
*   **分层错误转换**：
    *   **底层**：`pi-ai` 提供商层将 HTTP 错误、超时、JSON 解析错误转换为 `StopReason::Error` 并注入到消息事件中。
    *   **中层**：`pi-agent-core` 的 `agent_loop` 捕获这些事件，并根据错误类型决定是否终止循环或进入工具调用分支。
    *   **顶层**：`pi-coding-agent` 的 `main.rs` 和 `lib.rs` 将内部错误统一转换为 `CliOutput::failure`，通过 `stderr` 输出并设置非零退出码。
*   **RPC 协议错误标准化**：在 `protocol/rpc.rs` 中，所有内部错误都被序列化为标准的 JSON-RPC 风格响应 `RpcResponse::error`，包含 `id`、`type` 和 `message` 字段，确保跨语言客户端能一致地处理异常。
*   ** Panic 策略**：代码库在极少数涉及内部状态一致性且无法恢复的场景下使用 `panic!`（如 `agent.rs` 中检测到并发调用 `prompt()`），但在绝大多数 I/O 和业务逻辑中严格避免 panic，转而返回 `Result`。

### 4. 开发者规范
*   **定义新错误**：应在对应模块的 `error.rs` 文件中定义新的 `enum` 或 `struct`，并使用 `#[derive(thiserror::Error)]`。如果错误需要被上层逻辑区分处理，必须配套定义 `ErrorCode` 枚举。
*   **错误传播**：在异步函数中，优先使用 `?` 操作符传播错误。在流式处理中，严禁直接 `unwrap()`，应通过 `yield Event::Error` 将错误推送到流中。
*   **上下文保留**：在转换错误时（如从 `std::io::Error` 转为 `SessionError`），应使用 `format!("...: {}", e)` 保留原始错误信息，以便于调试。
*   **CLI 出口**：所有命令行入口点的错误最终都应映射为 `CliError`，并通过 `CliOutput` 结构体统一控制退出码和标准错误输出。