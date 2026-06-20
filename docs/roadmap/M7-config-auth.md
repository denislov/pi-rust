# M7 — 配置 + 认证基座（Rust 原生）

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：无 · 解锁：M8、M10、M11
> 定位：**核心地基**。多 provider 实际可用、其它子系统的配置来源，都依赖这一层。

## 目标
为 pi-rust 建立 Rust 原生的配置与认证基座。**不要求**读取 pi 的 `settings.json`/`auth.json`
（按决策：配置/认证用 Rust 原生格式，仅会话与 pi 互通）。

## 实际推进状态

M7 已按 Rust 原生 TOML 方案推进完成。当前约定：

- 全局配置目录：`PI_RUST_DIR` 覆盖，否则 `~/.pi-rust/`。
- 项目配置目录：`<cwd>/.pi-rust/`。
- settings 文件：`settings.toml`；auth 文件：`auth.toml`。
- API key 解析优先级：`--api-key` > provider 环境变量 > `auth.toml`。
- 不读取、不迁移 pi TypeScript 版本的 `~/.pi/agent/settings.json` / `auth.json`。

## 已实现项

### 1. settings 管理（Rust 原生）
- 全局 `~/.pi-rust/settings.toml` + 项目级 `<cwd>/.pi-rust/settings.toml` 合并（优先级：项目 > 全局 > 默认）。
- 字段：默认 provider/model、compaction 阈值、retry 配置、terminal（图像显示、进度指示）、theme 选择、session dir、context files 开关、skills/prompts/themes 资源路径。
- 已接入运行路径：
  - `default_provider` / `default_model` 参与 print/json、interactive、RPC 的模型选择回退。
  - `session_dir` 作为 `--session-dir` 的配置默认值。
  - `no_context_files` 作为 `--no-context-files` 的配置侧落点。
  - `compaction` 写入 `AgentConfig.compaction`。
  - `retry` 写入 `StreamOptions.max_retries` / `max_retry_delay_ms`。
  - `skills` / `prompts` / `themes` 作为资源加载补充路径。
  - `theme` 在资源加载层解析为 `selected_theme`，后续 UI 主题应用可直接消费。
- TS 参考：`coding-agent/src/core/settings-manager.ts`（~300 行）——**仅借鉴语义，格式按 Rust 原生设计**。
- Rust 实现：`pi-coding-agent::config::settings`，用 serde + 类型化 struct，缺省值在 resolve 阶段填充。

### 2. 认证存储（Rust 原生 auth.toml）
- `auth.toml` 存 API key（OAuth token 字段预留给 [M8](M8-provider-breadth.md)）。
- 20+ provider 环境变量解析：把 `pi-ai` 已有的 `env_keys.rs`（覆盖 30+ env）接线到 coding-agent 的 key 解析链。
- 解析优先级：`--api-key` > env > `auth.toml`。
- 支持 `$VAR` / `${VAR}` 环境变量替换、`$$` / `$!` 转义、Unix `0600` 权限 warning。
- 支持 `AuthStore::save()` TOML 写入和读写 round-trip。
- TS 参考：`coding-agent/src/core/auth-storage.ts`（~300 行）。

### 3. CLI 接线
- `--model`/`--provider` 经由 settings 默认值回退，CLI 显式值优先。
- `--no-context-files` 已有配置侧落点。
- print/json、interactive、RPC 均加载同一套 M7 config/auth。

## 不在本里程碑
- OAuth 流程（→ [M8](M8-provider-breadth.md)，依赖 pi-ai OAuth）。
- 从 pi 迁移配置（决策：配置不与 pi 兼容，故无需迁移；内部版本迁移按需后置）。

## 验收 / 测试（离线优先）
- settings 合并：单测覆盖"项目覆盖全局覆盖默认"。
- key 解析：单测覆盖三级优先级；用临时目录 + 注入 env，不依赖真实 key。
- auth.toml 读写往返（serde round-trip）测试。
- runtime/config wiring 测试覆盖默认 provider、默认 model、session dir、context files、retry、compaction。
