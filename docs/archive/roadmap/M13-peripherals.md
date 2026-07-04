# M13 — 周边能力（后置）

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M11](M11-interactive-ux.md)
> 决策：**核心优先，周边后置**。本里程碑的项可按需切片，不阻塞核心。

## 目标
补齐 pi 的周边/生态能力，对标"完整功能"的收尾部分。

## 待实现项

### 1. 导出 / 分享
- ✅ **`/export` 基础导出**：Rust interactive 已支持将当前 transcript 导出为 JSONL 或 HTML；JSONL 使用 session v3 header + 线性 message entries，HTML 为轻量离线可读页面。
- **HTML 导出 parity**：TS 的完整 HTML 查看器仍未移植。TS：`coding-agent/src/core/export-html/`（8 文件，含 template.html/css/js）。
- ✅ **JSONL 导入**：`/import` 可打开指定 JSONL 并切换后续 prompt target；路径参数支持引号。
- **gist 分享**：`/share` 经 GitHub gist 分享会话。TS：`modes/interactive/interactive-mode.ts`。
- ✅ **`/copy`**：复制最后一条 assistant 消息到剪贴板；已抽象 `ClipboardSink`，测试用内存实现，不依赖系统剪贴板。
- ✅ **`/clone` 基础克隆**：可基于当前 active session leaf 生成带 `parentSession` 的克隆 JSONL，并切换后续 prompt target。

### 2. 包管理 CLI（需先决策）
- pi 的 `pi install/remove/update/list/config`（npm/git 安装扩展）。TS：`coding-agent/src/package-manager-cli.ts`。
- **决策点**：Rust 侧不走 npm。可选：
  - (a) 用 cargo / 预编译二进制分发扩展；
  - (b) 仅管理 Lua 插件（与 [M12](M12-plugin-system.md) 对齐）；
  - (c) 暂缓，不做包管理 CLI。
- 建议先做 [M12](M12-plugin-system.md) 的 Lua 插件加载，包管理视实际需求再定。

### 3. 杂项
- ✅ `--list-models` 命令已实现，支持 provider 过滤、fuzzy search 和 JSON 输出。
- macOS 原生修饰键检测（pi 用预编译 `.node`）——**默认暂缓**，无合适 Rust 等价且价值低。

## 验收 / 测试（离线优先）
- HTML 导出：fixture 会话 → 断言关键 DOM 片段存在。
- gist：抽象出 sink/runner trait，测试不触网、不依赖 GitHub CLI。
