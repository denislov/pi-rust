## 1. 系统概述

该仓库采用**分层配置系统**，主要服务于 `pi-coding-agent`（智能体核心运行时）。配置来源按优先级从低到高依次为：
1. **全局配置文件** (`~/.pi-rust/settings.toml`, `auth.toml`)
2. **项目级配置文件** (`./.pi-rust/settings.toml`)
3. **环境变量** (Provider-specific API Keys, `PI_RUST_DIR`)
4. **命令行参数** (CLI Flags)

配置格式统一使用 **TOML**。系统支持配置的**合并（Merge）**与**覆盖（Override）**，并内置了诊断（Diagnostics）机制以报告配置加载过程中的警告或错误。

## 2. 核心组件与文件

### 2.1 配置加载器 (`pi-coding-agent/src/config/`)
- **`mod.rs`**: 定义 `Config` 结构体，包含 `Settings` 和 `AuthStore`。提供 `load_config(cwd)` 入口，协调路径解析、设置加载和认证加载。
- **`paths.rs`**: 负责解析配置目录。
  - **全局目录**: 优先读取 `PI_RUST_DIR` 环境变量；若未设置，则默认为 `$HOME/.pi-rust`。
  - **项目目录**: 固定为当前工作目录下的 `.pi-rust`。
- **`settings.rs`**: 处理 `settings.toml`。
  - 使用 `PartialSettings` (带 `Option` 字段) 进行反序列化，支持 `deny_unknown_fields` 以捕获拼写错误。
  - 实现 `merge` 逻辑：项目级配置会覆盖全局配置中已定义的字段，未定义的字段保留全局值。
  - 实现 `resolve` 逻辑：将 `PartialSettings` 转换为具有默认值的最终 `Settings` 结构体。
- **`auth.rs`**: 处理 `auth.toml`。
  - 支持 `api_key` 和 `oauth` 两种认证类型。
  - **环境变量插值**: 支持在 `auth.toml` 中使用 `$VAR` 或 `${VAR}` 引用环境变量，并在加载时解析。
  - **权限检查**: 在 Unix 系统上检查 `auth.toml` 权限是否为 `0600`，否则发出警告。

### 2.2 密钥解析 (`pi-ai/src/util/env_keys.rs`)
- **`env_api_key(provider)`**: 根据提供商名称（如 `anthropic`, `openai`）查找对应的标准环境变量（如 `ANTHROPIC_API_KEY`）。
- 支持多别名映射（例如 Anthropic 支持 `ANTHROPIC_API_KEY`, `CLAUDE_API_KEY` 等）。
- 对于 Bedrock/Vertex 等基于 IAM/ADC 的提供商，通过检查特定环境变量（如 `AWS_PROFILE`）的存在性返回哨兵值 `<authenticated>`。

### 2.3 命令行参数 (`pi-coding-agent/src/args.rs`)
- 手动实现参数解析器，支持 `--provider`, `--model`, `--api-key`, `--session-dir` 等标志。
- CLI 参数具有最高优先级，直接覆盖从文件和环境中加载的配置。

## 3. 架构与约定

### 3.1 密钥解析优先级
在 `config/auth.rs` 的 `resolve_api_key` 函数中定义了严格的密钥获取顺序：
1. **CLI 参数** (`--api-key`): 最高优先级，用于临时覆盖。
2. **环境变量** (`pi_ai::env_api_key`): 次高优先级，符合云原生/十二要素应用规范。
3. **认证文件** (`auth.toml`): 最低优先级，支持持久化存储和变量引用。

### 3.2 设置合并策略
- **标量字段**: 项目级非空值覆盖全局值。
- **列表字段** (`skills`, `prompts`, `themes`): 采用**追加（Append）**策略，即 `global + project`。
- **嵌套对象** (`compaction`, `retry`, `terminal`): 采用**字段级合并**，项目级仅覆盖显式指定的子字段，其余保留全局值。

### 3.3 诊断与容错
- 配置加载失败（如文件不存在、解析错误）通常不会导致程序崩溃，而是返回默认值并记录 `ConfigDiagnostic`。
- 诊断信息通过 `drain_diagnostics` 输出到 `stderr`，格式为 `config warning: <message> (<source>)`。

## 4. 开发者指南

### 4.1 添加新配置项
1. 在 `settings.rs` 的 `PartialSettings` 中添加 `Option<T>` 字段。
2. 在 `Settings` 中添加最终类型字段。
3. 在 `merge` 方法中处理合并逻辑（标量用 `or`，列表用 `merge_vec`）。
4. 在 `resolve` 方法中提供默认值。
5. 更新 `load_partial` 的测试用例以验证 TOML 解析。

### 4.2 安全最佳实践
- **严禁**在 `auth.toml` 中明文存储敏感密钥。应使用环境变量引用：
  ```toml
  [anthropic]
  type = "api_key"
  key = "$ANTHROPIC_API_KEY"
  ```
- 确保 `auth.toml` 文件权限设置为 `0600`。

### 4.3 环境变量规范
- `PI_RUST_DIR`: 覆盖全局配置目录路径。
- Provider Keys: 遵循 `pi-ai` 中定义的标准变量名（见 `env_keys.rs`）。

## 5. 关键文件清单
- `crates/pi-coding-agent/src/config/mod.rs`
- `crates/pi-coding-agent/src/config/settings.rs`
- `crates/pi-coding-agent/src/config/auth.rs`
- `crates/pi-coding-agent/src/config/paths.rs`
- `crates/pi-ai/src/util/env_keys.rs`
- `crates/pi-coding-agent/src/args.rs`
- `crates/pi-coding-agent/src/runtime.rs`