该项目采用标准的 Rust Cargo 工作区（Workspace）模式进行构建与依赖管理，未引入 Makefile、Docker 或外部 CI/CD 配置文件。

### 1. 构建系统与工具
- **核心工具**：使用 `cargo` 作为唯一的构建、测试和打包工具。
- **工作区结构**：根目录 `Cargo.toml` 定义了 workspace，包含 `pi-agent-core`、`pi-ai`、`pi-coding-agent`、`pi-tui` 等 7 个成员 crate。
- **编译标准**：所有 crate 均指定 `edition = "2024"`，表明项目紧跟 Rust 最新语言特性。
- **依赖锁定**：通过 `Cargo.lock` 确保构建的可重复性。

### 2. 关键文件与逻辑
- **`Cargo.toml` (根目录)**：定义工作区成员及顶层包 `pi-rust`。
- **`crates/*/Cargo.toml`**：各子模块独立管理其依赖。例如 `pi-ai` 负责网络请求（`reqwest`），`pi-tui` 负责终端渲染（`crossterm`）。
- **`scripts/tui-smoke.sh`**：项目中唯一的自动化脚本，用于执行 TUI（终端用户界面）的冒烟测试。该脚本利用 `tmux` 模拟用户交互并捕获屏幕输出，验证 UI 渲染的正确性。

### 3. 架构与约定
- **模块化构建**：采用多 crate 拆分架构，将 AI 提供商适配、智能体核心逻辑、TUI 渲染和业务入口解耦，便于并行开发与独立测试。
- **异步运行时**：核心业务 crate 普遍依赖 `tokio` 及其生态（`futures`, `async-stream`），构建产物为异步驱动的二进制或库。
- **测试策略**：除了常规的 `cargo test`，项目引入了基于 `tmux` 的端到端 UI 测试脚本，体现了对交互式终端应用稳定性的重视。

### 4. 开发者规范
- **构建命令**：使用 `cargo build -p <crate-name>` 构建特定模块，或使用 `cargo build` 构建整个工作区。
- **测试执行**：运行 `cargo test` 执行单元测试与集成测试；运行 `scripts/tui-smoke.sh` 进行 UI 回归测试（需安装 `tmux`）。
- **依赖管理**：新增依赖需在对应的子 crate `Cargo.toml` 中声明，保持依赖边界清晰。