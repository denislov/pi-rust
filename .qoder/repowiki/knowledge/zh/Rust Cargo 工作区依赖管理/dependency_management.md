该项目采用 Rust 官方的 **Cargo** 作为依赖管理系统，并基于 **Workspace（工作区）** 架构组织多个子包（Crates）。

### 1. 核心系统与工具
- **包管理器**: Cargo (Rust 标准构建与依赖工具)。
- **版本控制**: 使用 `Cargo.lock` 锁定所有传递性依赖的精确版本，确保构建的可重复性。
- **源配置**: 默认从 crates.io 公共注册表拉取依赖，未发现私有源或 Git 依赖配置。

### 2. 关键文件与结构
- **根目录 `Cargo.toml`**: 定义了工作区成员 (`members`)，包括 `pi-agent-core`, `pi-ai`, `pi-coding-agent`, `pi-tui` 等 7 个 Crates。
- **各 Crate `Cargo.toml`**: 
  - `pi-ai`: 核心 AI 交互层，依赖 `reqwest`, `tokio`, `serde` 等。
  - `pi-agent-core`: 智能体逻辑层，依赖 `pi-ai` 及 `futures`, `uuid` 等。
  - `pi-coding-agent`: 编码代理实现，依赖 `pi-agent-core`, `pi-tui`, `image` 等。
  - `pi-tui`: 终端用户界面，依赖 `crossterm`, `pulldown-cmark` 等。
- **`Cargo.lock`**: 包含约 2500 行依赖解析结果，记录了所有直接和间接依赖的校验和。

### 3. 架构与约定
- **路径依赖 (Path Dependencies)**: 内部模块间通过 `{ path = "../..." }` 进行链接，形成了清晰的层级关系（如 `pi-coding-agent` -> `pi-agent-core` -> `pi-ai`）。
- **统一版本策略**: 所有 Crate 均使用 `edition = "2024"`，表明项目紧跟 Rust 最新语言特性。
- **功能精简**: 在引入重型依赖（如 `reqwest`, `image`）时，普遍使用 `default-features = false` 并仅开启必要功能（如 `rustls-tls`, `json`），以优化编译体积和速度。

### 4. 开发者规范
- **依赖声明**: 新增第三方库时，应在对应 Crate 的 `Cargo.toml` 中声明，并尽量指定主要版本号（如 `"1"`, `"0.12"`）。
- **锁文件管理**: `Cargo.lock` 应提交至版本控制系统。修改依赖后需运行 `cargo update` 或 `cargo build` 自动更新锁文件。
- **内部引用**: 跨 Crate 调用必须通过工作区路径依赖实现，禁止循环依赖。