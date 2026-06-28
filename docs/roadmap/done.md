# 已完成（M0–M11）

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md)
> 本文件记录已落地的能力。完成信号以实际代码与 git 提交为准。

---

## ⚠️ 2026-06-20 审阅修正项

对 `pi-agent-core` 重新核验代码后，旧 ROADMAP 把以下项目误列为"待实现"，实际**已实现**：

| 项目 | 旧状态 | 实际 | 证据 |
|---|---|---|---|
| 并行工具执行 + `ToolExecutionMode` | ❌ 待实现 | ✅ 已实现，**默认 `Parallel`** | `crates/pi-agent-core/src/types.rs:52`（枚举）/`:329`（默认）；`agent_loop.rs:3` 用 `FuturesUnordered` |
| skills 加载 | ❌ 待实现 | ✅ 已实现 | `pi-agent-core/src/resources/skills.rs`；coding-agent `resources.rs` 已接线 |
| prompt templates（参数替换 `$1`/`$@`/`$ARGUMENTS`） | ❌ 待实现 | ✅ 已实现 | `pi-agent-core/src/resources/prompt_templates.rs`、`system_prompt.rs:46` |
| thinking level | ❌ 待实现 | ✅ 已实现 | `ThinkingLevel` 枚举 + `agent_loop.rs` 映射 token 预算 |

> 结论：`pi-agent-core` 的**运行时内核**比旧文档描述的更完整。

---

## M0 — 稳定化 ✅
- 修复 `lookup_deepseek_model` 失败；工作区测试全绿。

## M1 — 内置工具集 ✅
- 7 个工具 `read/write/edit/bash/grep/find/ls`，含输出截断、cwd、diff，接入 CLI。

## M2 — Provider 广度 + 模型注册表 ✅
- 线缆兼容核心类型 + serde 桥接；惯用法流式（`EventStream`/`complete()`/`CancellationToken`）。
- Provider trait + 按 `api` 键全局注册表。
- **5 个 provider**：Anthropic、DeepSeek、OpenAI Completions、OpenAI Responses、Google GenAI。
- 生成式模型注册表（`models_generated.rs`，~18.7K 行）。
- 客户端重试/超时（`http_retry.rs`）；env-key 解析（`env_keys.rs`，覆盖 30+ provider env）。
- faux provider（离线端到端测试）；流式 JSON 修复 + 部分解析。

## M3 — 会话持久化 ✅
- JSONL/内存存储、分支树、continue/resume/fork/clone、session id、cwd 关联。
- **约束**：session JSONL 需与 pi 保持互通（见 [cross-cutting.md](cross-cutting.md) 的兼容约束）。

## M4 — agent-core 运行时内核 ✅
- `Agent`（`Arc<RwLock<AgentState>>`）：`new`/`add_tool`/`add_message`/`messages`/`prompt`/`abort`。
- `run_loop`：工具调用循环（顺序+**并行**）、`max_turns`、取消传播、按 stop-reason 分支。
- compaction（上下文压缩 + token 计量）、会话持久化层、钩子、steering/follow-up 队列。
- skills/prompt templates 加载与系统提示注入、thinking level。

## M5 — headless 协议模式 ✅
- `--mode json` 事件流 + RPC（stdio JSON-RPC）。

## M6 — 交互式 TUI ✅
- 输入栈（raw mode、bracketed paste、Kitty 键盘协议、按键解析、keybindings）。
- `RenderScheduler`（~60Hz coalescing）、光标稳定（`CURSOR_MARKER`）、内联差分渲染。
- transcript 布局/滚动、生命周期（RAII guard、Ctrl+C 三路径）。

## M7 — 配置 + 认证基座（Rust 原生）✅
- TOML `settings.toml` + `auth.toml`，全局 `~/.pi-rust/` + 项目级合并。
- 20+ provider 环境变量解析，`$VAR` 替换，`0600` 权限。
- 三级 API key 优先级：`--api-key` > env > `auth.toml`。
- 全部运行路径（print/json/interactive/RPC）接线。
- 详见 [M7-config-auth](M7-config-auth.md)。

## M8 — pi-ai provider 广度 + 认证 ✅
- 新增 4 个 provider：Mistral、Azure OpenAI Responses、AWS Bedrock（SigV4）、OpenAI Codex Responses。
- OAuth 工具基础（PKCE + HTML pages），Cloudflare/Copilot 辅助。
- `Model.compat` / `thinkingLevelMap` 从不透明 Value 收敛为强类型。
- 图像生成 types + OpenRouter helper、diagnostics、content hash。
- 详见 [M8-provider-breadth](M8-provider-breadth.md)。

## M9 — agent-core harness 完备 ✅
- `AgentHarness` 包装层，6 个 harness 钩子，`StreamOptionsPatch` with Set/Clear/Merge。
- 自定义消息类型：`BashExecution`、`Custom`、`BranchSummary`。
- `FileSystem` / `Shell` / `ExecutionEnv` 抽象层，支持离线测试注入。
- 流式 proxy、branch summarization、类型化错误。
- 详见 [M9-agent-harness](M9-agent-harness.md)。

## M10 — 资源发现 + 输入路径 ✅
- `AGENTS.md`/`CLAUDE.md` 自动发现（祖先遍历 + 全局 agent dir）。
- skills/prompt templates/themes 自动发现，对应 `--no-*` 开关。
- `@file`/`@image.png` 输入、stdin 管道、`--models` 模型 glob 轮换。
- 补齐缺失 CLI flag：`--provider`/`--append-system-prompt`/`--tools`/`--exclude-tools`/`--verbose`/`--offline` 等。
- 详见 [M10-resources-input](M10-resources-input.md)。

## M11 — 交互体验补全（TUI 基础设施）✅
- Slash command registry（23 命令），`/help`/`/quit`/`/model` 等有本地行为。
- `pi-tui` 可复用组件：`Loader`/`CancellableLoader`、`Box`、`TruncatedText`、`SettingsList`、`Image`、`SelectorDialog`。
- Fuzzy 搜索 + autocomplete，256/RGB 主题系统（dark/light/custom）。
- ANSI-aware wrapping/truncation，Kitty/iTerm2 终端图像协议。
- TUI-7 smoke suite（`scripts/tui-smoke.sh`）。
- 详见 [M11-interactive-ux](M11-interactive-ux.md)。
