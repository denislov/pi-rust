## 1. 系统/方法
当前仓库**未集成**专用的 Rust 日志框架（如 `tracing`、`log`、`env_logger` 或 `slog`）。日志输出完全依赖 Rust 标准库的宏：
- `println!`：用于示例代码（examples）中的流程演示和最终结果展示。
- `eprintln!`：用于生产代码（src/）中的错误报告和关键状态提示，直接写入标准错误流（stderr）。
- `dbg!`：未在核心逻辑中发现使用，表明调试信息主要通过错误路径或交互式 UI 反馈。

## 2. 关键文件与位置
由于缺乏集中式日志模块，日志行为分散在各个 crate 的入口点和错误处理路径中：
- `crates/pi-coding-agent/src/main.rs`：CLI 入口，使用 `eprintln!` 输出 RPC 模式错误和 stdin 读取失败信息。
- `crates/pi-ai/src/providers/openai/responses/process.rs`：在 SSE 解析失败时使用 `eprintln!` 输出原始数据片段。
- `crates/pi-coding-agent/src/resources.rs`：资源加载异常时使用 `eprintln!`。
- `crates/*/examples/*.rs`：大量使用 `println!` 进行功能演示。

## 3. 架构与约定
- **无结构化日志**：所有输出均为纯文本字符串，不包含时间戳、日志级别、模块路径或追踪 ID 等结构化字段。
- **无日志路由**：没有日志级别过滤（如 INFO/DEBUG/ERROR），也没有将日志写入文件或远程端点的能力。
- **错误即日志**：在非交互模式下，`stderr` 是唯一的“日志”通道，主要用于向调用者（如脚本或父进程）传达致命错误。
- **UI 与日志分离**：在 `pi-tui` 和 `pi-coding-agent` 的交互模式中，用户反馈通过 TUI 组件渲染，而非传统日志行。

## 4. 开发者应遵循的规则
- **禁止在生产代码中使用 `println!`**：所有面向用户的错误或状态信息应通过 `eprintln!` 输出到 stderr，或通过返回 `Result` 由上层统一处理。
- **错误处理优先**：优先使用 `thiserror` 定义结构化错误类型，仅在顶层捕获处或不可恢复错误时打印日志。
- **示例代码例外**：在 `examples/` 目录下可以使用 `println!` 以增强可读性。
- **未来演进建议**：若需增加可观测性，建议引入 `tracing` 生态，并在 `main.rs` 初始化 `tracing-subscriber`，以便支持结构化日志和异步上下文追踪。