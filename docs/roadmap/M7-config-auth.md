# M7 — 配置 + 认证基座（Rust 原生）

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：无 · 解锁：M8、M10、M11
> 定位：**核心地基**。多 provider 实际可用、其它子系统的配置来源，都依赖这一层。

## 目标
为 pi-rust 建立 Rust 原生的配置与认证基座。**不要求**读取 pi 的 `settings.json`/`auth.json`
（按决策：配置/认证用 Rust 原生格式，仅会话与 pi 互通）。

## 待实现项

### 1. settings 管理（Rust 原生）
- 全局 `~/.pi/agent/settings.json` + 项目级 settings 合并（优先级：项目 > 全局 > 默认）。
- 字段：默认 provider/model、compaction 阈值、retry 配置、terminal（图像显示、进度指示）、theme 选择。
- TS 参考：`coding-agent/src/core/settings-manager.ts`（~300 行）——**仅借鉴语义，格式按 Rust 原生设计**。
- Rust 目标：`pi-coding-agent` 新增 `settings` 模块；用 serde + 类型化 struct，缺省值用 `Default`。

### 2. 认证存储（Rust 原生 auth.json）
- `auth.json` 存 API key（OAuth token 字段预留给 [M8](M8-provider-breadth.md)）。
- 20+ provider 环境变量解析：把 `pi-ai` 已有的 `env_keys.rs`（覆盖 30+ env）接线到 coding-agent 的 key 解析链。
- 解析优先级：`--api-key` > env > auth.json。
- TS 参考：`coding-agent/src/core/auth-storage.ts`（~300 行）。

### 3. CLI 接线
- 让 `--model`/`--provider`（[M10](M10-resources-input.md) 增加 `--provider`）经由 settings 默认值回退。
- `--no-context-files` 等开关的配置侧落点（开关本体在 M10）。

## 不在本里程碑
- OAuth 流程（→ [M8](M8-provider-breadth.md)，依赖 pi-ai OAuth）。
- 从 pi 迁移配置（决策：配置不与 pi 兼容，故无需迁移；内部版本迁移按需后置）。

## 验收 / 测试（离线优先）
- settings 合并：单测覆盖"项目覆盖全局覆盖默认"。
- key 解析：单测覆盖三级优先级；用临时目录 + 注入 env，不依赖真实 key。
- auth.json 读写往返（serde round-trip）测试。
