# M10 — 资源发现 + 输入路径

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)（settings）· 解锁：[M11](M11-interactive-ux.md)
> 定位：**核心 UX**。让 coding-agent 能自动发现上下文、接受多种输入来源。

## 目标
补齐 `pi-coding-agent` 的资源自动发现与输入路径，对标 pi 的 CLI 入参体验。

## 待实现项

### 1. 上下文文件发现
- ✅ 发现并加载 `AGENTS.md` / `CLAUDE.md`：祖先目录向上遍历 + 全局 agent dir，按全局、root、leaf 优先级合并。
- ✅ `--no-context-files` 开关。TS：`coding-agent/src/core/resource-loader.ts`（`loadProjectContextFiles`）。

### 2. skills / templates / themes 自动发现
- ✅ 从 settings + 默认路径自动发现 skills / prompt templates / themes。
  - settings 字段：`skills`、`prompts`、`themes`。
  - 默认路径：global/project `skills`、`prompts`，并兼容 Rust 早期的 `prompt-templates` 目录；themes 加载 `.json` 文件。
- ✅ 开关：`--no-skills`、`--no-prompt-templates`、`--no-themes`。
- TS：`coding-agent/src/core/resource-loader.ts`。

### 3. 输入路径
- ✅ `@file` 引用：把文件内容注入首条消息，支持相对路径、`~`、以及 `@"path with spaces"` / `@'path with spaces'`。
- ✅ 图像输入：`@image.png`，含 mime 探测、base64 编码、默认 2000px 最大边 resize；多模态 `ContentBlock::Image` 已进入 provider context。
- ✅ stdin 管道：binary 入口检测 pipe vs TTY，读取并拼接到 prompt；测试覆盖伪 stdin merge。
- TS：`file-processor.ts` + `utils/image-resize.ts`/`utils/mime.ts`。

### 4. `--models` 模型轮换
- ✅ `--models a,b,c`（逗号分隔），支持 glob（`anthropic/*`、`*sonnet*`）与 thinking 绑定（`sonnet:high,haiku:low`）。
- ✅ headless model selection 已按 `--provider` + `--models` 选择匹配模型。
- ⏭️ 交互模式 Ctrl+P / Ctrl+Shift+P 循环切换留到 [M11](M11-interactive-ux.md)。TS：`cli/args.ts:113`、`core/keybindings.ts:76-83`。

### 5. 缺失 CLI flag 补全
- ✅ `--provider`、`--append-system-prompt`（可重复）、`--tools/-t`、`--exclude-tools/-xt`、`--no-tools`、`--no-builtin-tools`、`--verbose`、`--offline`。
- ✅ 工具过滤逻辑（allowlist/denylist）接入工具分发。
- ✅ `--no-builtin-tools` 仅过滤内置工具，保留自定义工具；`--no-tools` 清空全部工具。

## 验收 / 测试（离线优先）
- ✅ 上下文发现：临时目录树构造 + 断言合并顺序。
- ✅ 资源发现：默认路径、settings 路径、disable switches、theme JSON discovery。
- ✅ `@file`/图像：fixture 文件 + 断言注入的消息内容/编码；resize 测试断言 4x2 PNG 缩到 2x1。
- ✅ 多模态运行路径：recording provider 断言 `ContentBlock::Image` 到达 provider context。
- ✅ stdin：注入伪 stdin，断言拼接结果。
- ✅ `--models` glob：表驱动单测 + provider/model selection 单测。
- ✅ CLI flag / 工具过滤：allowlist、denylist、`--no-tools`、`--no-builtin-tools`。

## 本轮落地
- 新增 `input.rs` 多源 prompt 处理：`@file`、quoted path、image mime/base64/resize、stdin merge。
- 新增 `models.rs` model rotation parser，`runtime::select_model` 接入 `--provider` / `--models`。
- 扩展 settings：`skills`、`prompts`、`themes`。
- 扩展 resource loader：settings + default discovery、prompt `prompts` alias、theme JSON loading。
- 扩展 run path：图片 prompt 使用 `PromptInvocation::Content`，通过 `AgentMessage::Custom` 进入 `pi-agent-core` context。
- 补齐 M10 CLI flags 与工具过滤语义。
