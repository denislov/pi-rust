# 已完成（M0–M6 + TUI-8 部分）

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md)
> 本文件记录已落地的能力，并附 **2026-06-19 审阅修正项**。完成信号以实际代码与 git 提交为准；
> `docs/superpowers/plans/*.md` 的复选框从未勾选，**不能**作为完成信号。

---

## ⚠️ 2026-06-19 审阅修正项

对 `pi-agent-core` 重新核验代码后，旧 ROADMAP 把以下项目误列为"待实现"，实际**已实现**：

| 项目 | 旧状态 | 实际 | 证据 |
|---|---|---|---|
| 并行工具执行 + `ToolExecutionMode` | ❌ 待实现 | ✅ 已实现，**默认 `Parallel`** | `crates/pi-agent-core/src/types.rs:52`（枚举）/`:329`（默认）；`agent_loop.rs:3` 用 `FuturesUnordered` |
| skills 加载 | ❌ 待实现 | ✅ 已实现 | `pi-agent-core/src/resources/skills.rs`；coding-agent `resources.rs` 已接线 |
| prompt templates（参数替换 `$1`/`$@`/`$ARGUMENTS`） | ❌ 待实现 | ✅ 已实现 | `pi-agent-core/src/resources/prompt_templates.rs`、`system_prompt.rs:46` |
| thinking level | ❌ 待实现 | ✅ 已实现 | `ThinkingLevel` 枚举 + `agent_loop.rs` 映射 token 预算 |

> 结论：`pi-agent-core` 的**运行时内核**比旧文档描述的更完整。其剩余缺口是更上层的
> **harness 包装层**与**抽象层**（见 [M9](M9-agent-harness.md)），而非上述项。

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

## M4 — agent-core harness 能力 ✅
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

## TUI-8 — 交互 polish（部分 ✅）
- ✅ 配色（8 色语义化、NO_COLOR/TERM=dumb 禁用、transcript 角色着色）。
- ✅ Markdown 渲染（标题加粗、行内 code reverse、代码块/引用 dim、规则）。
- ✅ spinner（braille 动画，目前**硬编码在 pi-coding-agent**，尚未抽成 pi-tui 公共组件 → 见 [M11](M11-interactive-ux.md)）。
- ❌ SelectList 菜单、主题系统 → 已并入 [M11](M11-interactive-ux.md)。
