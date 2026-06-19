# M10 — 资源发现 + 输入路径

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)（settings）· 解锁：[M11](M11-interactive-ux.md)
> 定位：**核心 UX**。让 coding-agent 能自动发现上下文、接受多种输入来源。

## 目标
补齐 `pi-coding-agent` 的资源自动发现与输入路径，对标 pi 的 CLI 入参体验。

## 待实现项

### 1. 上下文文件发现
- 发现并加载 `AGENTS.md` / `CLAUDE.md`：祖先目录向上遍历 + 全局 `~/.pi/agent/AGENTS.md`，按优先级合并。
- `--no-context-files` 开关。TS：`coding-agent/src/core/resource-loader.ts`（`loadProjectContextFiles`）。

### 2. skills / templates / themes 自动发现
- 从 settings + 默认路径自动发现（当前只能显式 `--skills/--skill` 传参）。
- 开关：`--no-skills`、`--no-prompt-templates`、`--no-themes`。
- TS：`coding-agent/src/core/resource-loader.ts`。

### 3. 输入路径
- `@file` 引用：把文件内容注入首条消息。TS：`coding-agent/src/cli/file-processor.ts`。
- 图像输入：`@image.png`，含 resize、mime 探测、base64 编码。TS：`file-processor.ts` + `utils/image-resize.ts`/`utils/mime.ts`。
- stdin 管道：检测 pipe vs TTY，读取并拼接到 prompt。TS：`coding-agent/src/main.ts`（`readStdinAsync`）。

### 4. `--models` 模型轮换
- `--models a,b,c`（逗号分隔），支持 glob（`anthropic/*`、`*sonnet*`）与 thinking 绑定（`sonnet:high,haiku:low`）。
- 交互模式 Ctrl+P / Ctrl+Shift+P 循环切换（UI 部分在 [M11](M11-interactive-ux.md)）。TS：`cli/args.ts:113`、`core/keybindings.ts:76-83`。

### 5. 缺失 CLI flag 补全
- `--provider`、`--append-system-prompt`（可重复）、`--tools/-t`、`--exclude-tools/-xt`、`--no-tools`、`--no-builtin-tools`、`--verbose`、`--offline`。
- 工具过滤逻辑（allowlist/denylist）接入工具分发。

## 验收 / 测试（离线优先）
- 上下文发现：临时目录树构造 + 断言合并顺序。
- `@file`/图像：fixture 文件 + 断言注入的消息内容/编码。
- stdin：注入伪 stdin，断言拼接结果。
- `--models` glob：表驱动单测。
