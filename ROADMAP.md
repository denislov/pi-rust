# pi → Rust 移植路线图 (ROADMAP)

> 状态：PoC 阶段 · 最后核对：2026-06-04
> 适用范围：`pi-rust/` 工作区（把 TypeScript `pi` monorepo 移植为 Rust）
> 参考基准：`pi/`（上游 TS 实现，行为权威来源）

本文件回答三个问题：**已经实现了什么**、**还差什么**、**接下来按什么顺序做**。
它是对 `docs/superpowers/specs|plans` 各阶段设计文档的上层汇总，用于指导后续工作。

---

## 1. 总览

`pi` 是一个编码 agent harness，TS 侧约 ~98K 行源码（不含生成代码与测试），由四个核心包组成。
当前 Rust 侧完成了**四个核心 crate 各自的第一个 PoC 垂直切片**，验证了移植路径上最难的几个问题
（异步 SSE 流、serde 线缆兼容、trait 多态、agent 工具循环、差分渲染），但功能覆盖仍处早期。

### 1.1 依赖关系

```
pi-ai  ──┬──>  pi-agent-core  ──>  pi-coding-agent
         │                              ▲
pi-tui ──┴──────────────────────────────┘   (交互模式时接入)
```

- `pi-ai` 是依赖根，其它都建立在它之上。
- `pi-tui` 独立，可与 provider 工作并行推进；最终供 `pi-coding-agent` 交互模式使用。
- `pi-mom` / `pi-pods` / `pi-web-ui` 不在 AGENTS.md 的四包迁移图内，目前是空壳，**范围未定义**。

### 1.2 规模与完成度（已核实）

| 包 / crate | TS 源码 | Rust 源码 | 测试函数 | 粗略覆盖 | PoC 状态 |
|---|---:|---:|---:|---:|---|
| `pi-ai` | 30,553 | 2,715 | 69 | ~9% | ✅ PoC 完成（+ DeepSeek 进行中） |
| `pi-agent-core` | 8,067 | 586 | 17 | ~7% | ✅ PoC 完成 |
| `pi-coding-agent` | 48,013 | 361 | 27 | ~1% | ✅ print-mode 切片完成 |
| `pi-tui` | 11,696 | 660 | 21 | ~6% | ✅ 渲染地基完成 |
| `pi-mom` | — | 14 | 0 | — | ⬜ 空壳（`cargo new` 桩） |
| `pi-pods` | — | 14 | 0 | — | ⬜ 空壳 |
| `pi-web-ui` | — | 14 | 0 | — | ⬜ 空壳 |
| **合计** | **~98K** | **~4.4K** | **134** | **~4.5%** | — |

> “TS 源码”为 `src/**/*.ts`（去除 `*.test.ts`）行数；`pi-ai` 含 ~16.3K 行生成的模型注册表。

### 1.3 当前健康度 ⚠️

- 工作区可编译通过（Rust edition 2024）。
- **`cargo test --workspace` 当前为红**：`pi-ai` 有 **1 个失败测试**
  `models::tests::lookup_deepseek_model`（`crates/pi-ai/src/models.rs:190`）。
  原因：测试期望模型 `deepseek-chat`（api `deepseek-chat-completions`），但模型表里只有
  `deepseek-v4-flash` / `deepseek-v4-pro`——这是一处**尚未完成的 DeepSeek provider 扩展**
  （超出 pi-ai 原始 PoC spec，spec 只要求 Anthropic + faux）。
  **修复它是第一优先级**（见 M0）。
- 各阶段 `plans/*.md` 的复选框从未勾选（全为 `[ ]`），**不能**作为完成信号；以实际代码与 git 提交为准。

---

## 2. 各 crate 现状

### 2.1 `pi-ai` — 多 provider LLM 统一 API

**已实现（PoC spec 范围内）**
- 线缆兼容的核心类型：`ContentBlock` / `Message` / `AssistantMessage` / `Usage` / `Cost` /
  `StopReason` / `AssistantMessageEvent` / `Context` / `Tool` / `Model` / `StreamOptions` / `ThinkingConfig`，
  serde 桥接到 pi 的精确 JSON（tagged union、camelCase）。
- 惯用法流式模型：`EventStream`（`async-stream` 拉取式，无 channel/spawn）、`complete()`、
  经 `tokio-util::CancellationToken` 的取消；“不抛错”契约（失败编码为终止 `Error` 事件）。
- Provider trait + 按 `api` 键的全局注册表。
- **Anthropic provider（API-key 路径）**：请求构建（system + `cache_control`、消息转换、
  连续 tool-result 合并、tools→`input_schema`、`max_tokens`、`temperature` 门控、thinking、
  `tool_choice`、图像输入、tool-call id 规范化）；SSE 解码器；wire 结构；`process.rs` 事件映射
  （text / thinking / redacted_thinking / tool_use）；用量累计 + 成本计算；stop-reason 映射。
- 流式 JSON 修复 + 部分解析（增量 tool 参数）。
- 环境变量 key 解析；手写的小型 Anthropic 模型表。
- **faux provider**（离线端到端；已增强为 per-call 队列 + 自定义 stop_reason 以驱动 agent 循环）。

**进行中（超出 spec，未完成）**
- 🟡 **DeepSeek provider**：`providers/deepseek/{mod,convert,process,wire}.rs` 已存在，
  但模型表条目与测试期望不一致，导致测试失败。需对齐并补全。

**待实现（vs TS ~30.5K）**
- ❌ 其它 provider API：OpenAI completions / responses / codex、Google GenAI、Google Vertex、
  AWS Bedrock、Mistral、Azure OpenAI（TS 内置 9 类 API、已知 25 个 provider）。
- ❌ 生成式模型注册表（TS ~16.3K 行 / 100+ 模型）——Rust 仅手写约 12 个。
- ❌ 客户端重试 / 超时配置（`maxRetries` / `maxRetryDelayMs` / 各类 timeout）。
- ❌ 结构化诊断（diagnostics）、session affinity 头、`metadata`/`user_id` 透传。
- ❌ OAuth / Claude-Code 身份 / GitHub Copilot / Cloudflare 网关 / Bedrock SigV4 认证。
- ❌ provider 兼容标志矩阵（Fireworks / z.ai / OpenRouter / Vercel AI Gateway / Qwen 等）。
- ❌ 图像*生成* API（`images.ts`）；跨 provider 的多模态图像输入（Anthropic 图像输入已部分覆盖）。

### 2.2 `pi-agent-core` — agent 运行时（工具循环 + 状态）

**已实现（PoC spec 范围内）**
- `Agent`（`Arc<RwLock<AgentState>>`）：`new` / `add_tool` / `add_message` / `messages` / `prompt` / `abort`。
- `AgentMessage`（UserText / Assistant / ToolResult / SystemPrompt）、`AgentTool`（含 `ToolFn` 异步执行）、
  `AgentConfig`、`AgentEvent`（TurnStart / LlmEvent 透传 / ToolCallStart-End / AgentDone / AgentError）。
- `run_loop`：**顺序**工具调用循环、`max_turns`、取消传播、按 stop-reason 分支、各分支落库 assistant/tool 消息。
- `convert_to_context`（AgentMessage → pi-ai `Context`）；测试用 `ApiProvider`。

**待实现（vs TS ~8K，TS 体量约为当前 14.7×）**
- ❌ 并行工具执行 + `ToolExecutionMode`；`continue_loop()`。
- ❌ 钩子：`beforeToolCall` / `afterToolCall` / `shouldStopAfterTurn` / `prepareNextTurn`。
- ❌ steering / follow-up 消息队列（“all” vs “one-at-a-time”）。
- ❌ **上下文压缩 compaction**（token 计量、摘要、分支摘要；TS harness/compaction ~1.16K 行）。
- ❌ **会话持久化**（JSONL/内存存储、分支树、session repo；TS session ~1.02K 行）。
- ❌ skills 加载、prompt templates（参数替换）。
- ❌ 自定义消息类型（BashExecution / Custom / BranchSummary / CompactionSummary）。
- ❌ thinking level、proxy 流式（服务端路由）、`FileSystem`/`Shell` 抽象、类型化错误类、整个 harness 层。

### 2.3 `pi-coding-agent` — 编码 agent CLI（**缺口最大**）

**已实现（PoC = print-mode 垂直切片）**
- library-first 结构：`run_cli()`；瘦 `main.rs`（结果 → 退出码）。
- 最小参数解析：`-p`/`--print`、位置参数 prompt、`--model`、`--api-key`、`--system-prompt`、
  `--max-turns`、`--help`/`-h`、`--version`/`-v`；类型化 `CliError` + 退出码。
- print 模式（仅文本）：跑一轮 prompt 经 agent 输出最终 assistant 文本；`Stop`/`Length` 返回 0，错误返回非 0。
- runtime：`lookup_model` 解析模型、默认 `claude-sonnet-4-5`、`register_builtins`、构建 `AgentConfig`；
  测试注入接缝（faux provider / 自定义工具）。

**待实现（vs TS ~48K）**
- ❌ **内置工具集（7 个）**：`read` / `write` / `edit` / `bash` / `grep` / `find`(glob) / `ls`。
  ← 这是把 PoC 变成“真正能干活的编码 agent”的关键一跳。
- ❌ JSON 流式模式（`--mode json`）、RPC 模式（stdio 上 JSON-RPC）。
- ❌ **交互式 TUI 模式**（整套 interactive 子系统，TS 约 36 个 UI 组件）。
- ❌ **会话管理**：JSONL 持久化、continue/resume/fork/clone/tree、session id、cwd 关联。
- ❌ settings 管理（全局 + 项目级合并）、auth 存储（auth.json / OAuth / 20+ provider env）。
- ❌ 扩展系统（loader / hooks / 自定义工具 / slash 命令 / UI / keybindings）、21 个内置 slash 命令。
- ❌ skills / prompt templates / themes / 资源加载（npm/git 包）、上下文文件发现（AGENTS.md/CLAUDE.md）。
- ❌ HTML 导出 / 分享、keybindings 配置、迁移逻辑。
- ❌ thinking level flag、provider 选择、模型轮换（`--models`）、scoped models、`@file`、图像输入、stdin 管道。

### 2.4 `pi-tui` — 终端 UI（差分渲染）

**已实现（PoC = 渲染地基）**
- `Component` trait + `Container`。
- `Tui<T: Terminal>` 渲染管理器：差分渲染（首个变化行）、全量重绘（首帧/尺寸变化/收缩）、
  同步输出（CSI 2026）、每行 SGR/OSC8 复位后缀、`LineTooWide` 先校验后写入。
- `Terminal` trait + `TerminalSize`、`ProcessTerminal`（crossterm）、`VirtualTerminal`（测试后端）。
- `Text` / `Spacer` 组件。
- 宽度工具：`visible_width`（跳过 ANSI/OSC/APC、tab=3、grapheme + unicode-width）、
  `truncate_to_width`（不切断 grapheme）。
- 明确决策：**不使用 ratatui**，保持 pi 的“组件返回字符串、框架拥有差分输出”哲学。

**待实现（vs TS ~11.7K）**
- ❌ **输入栈**：raw mode、stdin 缓冲、bracketed paste、Kitty 键盘协议、按键解析、keybindings。
- ❌ 事件循环 / 异步渲染 / `request_render`；焦点管理 + overlay/模态栈。
- ❌ 组件：`Input`、`Editor`（TS 单文件 2231 行）、`SelectList`、`Markdown`、`Image`、
  `Loader`/`CancellableLoader`、`Box`、`TruncatedText`。
- ❌ 硬件光标 / IME marker；autocomplete、fuzzy、kill-ring、undo/redo、词导航。
- ❌ 终端图像协议（Kitty/iTerm2）+ 能力探测；超链接（OSC 8）、进度（OSC 9;4）；主题系统。
- ❌ emoji 探测 / 宽度缓存 / ANSI 感知的高级换行。

### 2.5 `pi-mom` / `pi-pods` / `pi-web-ui` — 范围未定义

- 三者均为 14 行 `cargo new` 桩，无设计文档，不在 AGENTS.md 的四包迁移图内。
- **行动项**：在投入前先明确它们对应 TS 侧的什么（或上游新需求），否则保持空壳。

---

## 3. 路线图（里程碑）

下列里程碑按**“尽快得到一个可用的 headless 编码 agent”**这一推荐目标排序——它延续了 print-mode PoC 的轨迹、
风险低、价值高。若团队目标改为“尽早达到交互式 parity”，则把 **M6（TUI）**前移。
每个里程碑沿用既有原则：**离线确定性测试**、**serde 线缆兼容**、**惯用 Rust**、**小 crate 对应 TS 边界**、
**先只做 API-key 认证**，并遵循各自的 spec→plan→实现循环。

### M0 — 稳定化（立即，~0.5 天）
- 修复 `lookup_deepseek_model` 失败：对齐 DeepSeek 模型表 / api 名，或补全 DeepSeek provider 并加测试。
- 让 `cargo fmt --check`、`cargo test --workspace`、`cargo check --workspace` 全绿。
- （可选）加一个最小 CI / 校验脚本固化三项检查。
- **验收**：工作区测试全绿。

### M1 — 内置工具集（让它“能干活”，高优先级）
- 在 `pi-coding-agent` 实现 7 个工具：先 `read` / `write` / `edit` / `bash`，再 `grep` / `find`(glob) / `ls`；
  含输出截断、cwd、diff 输出等行为，对照 TS `core/tools/*.ts`。
- 依赖：`pi-agent-core`（已就绪）。建议每个工具配离线测试（临时目录 fixture）。
- **验收**：print 模式能跑通“读文件 → 编辑 → 跑命令”的多轮工具循环。

### M2 — Provider 广度 + 模型注册表
- 优先 OpenAI（completions/responses）与 Google GenAI；移植或生成 `models.generated` 注册表；
  扩展 env-key 解析；加入客户端重试/超时。
- 依赖：`pi-ai` 现有 provider 抽象（已就绪）。
- **验收**：可经至少一个非 Anthropic provider 完成 `complete()`，模型查找覆盖主流模型。

### M3 — 会话持久化
- `pi-agent-core` 补 session 层（JSONL/内存存储、分支树）；`pi-coding-agent` 补 session manager 与
  `--session`/`--continue`/`--resume`/`--fork` 等 flag、session id、cwd 关联。
- **验收**：CLI 可跨次运行续接同一会话；会话文件与 pi 的 JSONL 格式兼容（v3）。

### M4 — agent-core harness 能力
- compaction（上下文压缩 + token 计量）、并行工具执行、`beforeToolCall`/`afterToolCall` 钩子、
  steering/follow-up 队列、thinking level、skills / prompt templates。
- **验收**：长会话自动压缩；工具可并行；钩子可拦截/改写工具调用。

### M5 — headless 协议模式
- `--mode json` 事件流 + RPC（stdio JSON-RPC）模式，便于外部集成与端到端自动化测试。
- **验收**：外部进程可通过 JSON 事件流驱动一次完整会话。

### M6 — 交互式 TUI（大块，可与 M2–M5 并行起步）
- `pi-tui` 输入栈（raw mode / stdin 缓冲 / Kitty 键盘协议 / 按键解析 / keybindings）→
  `Input` / `Editor` / `SelectList` / `Markdown` 组件 → 焦点 / overlay → 异步渲染循环；
  最后把 `pi-coding-agent` 交互模式接到 `pi-tui` 上。
- **验收**：可启动交互式会话，键入、滚动、查看 Markdown/工具输出。

### M7 — 周边能力
- 扩展系统（loader + hooks + 自定义工具 + slash 命令 + UI/keybindings）、auth 存储 / OAuth、
  settings 管理、themes、HTML 导出 / 分享、资源加载、迁移逻辑、剩余 provider 与兼容矩阵。

### 横切项 — 辅助 crate 定位
- 为 `pi-mom` / `pi-pods` / `pi-web-ui` 明确目标范围（或决定暂缓 / 移除）。

> **可并行性**：M6（TUI 输入栈）与 M2（provider 广度）互不依赖，可由第二条工作线随时启动；
> M1（工具）是把 PoC 变成实用工具的最短路径，建议最先做。

---

## 4. 建议的近期下一步（可立即执行）

1. **M0**：修 `lookup_deepseek_model`，恢复绿色测试套件。
2. **M1 起步**：为 `pi-coding-agent` 写 `read` + `bash` 两个工具的 spec→plan→实现（带临时目录 fixture 测试），
   验证“CLI 经 agent 循环驱动真实工具”的端到端路径。
3. **补完 DeepSeek**：既然已动工，顺势把 DeepSeek provider 流式路径与模型表补齐并测试（巩固 M0）。
4. 为每个里程碑在 `docs/superpowers/specs` 新建设计文档，延续既有评审流程。

---

## 5. 关键决策与约束（沿用各 spec）

- **离线优先**：所有测试不依赖真实 provider key；用 faux provider / fixture / 单元测试证明正确性。
- **线缆兼容**：事件协议与 wire JSON 与 pi 保持字节级一致（serde 桥接），为后续 interop（pi 会话文件等）降险。
- **惯用 Rust**：snake_case + enum + `Result`/typed error，而非照搬 TS 结构。
- **小 crate 对应 TS 包边界**，不向根 package 堆叠跨切面代码。
- **认证先只做 API-key 路径**；OAuth / Bedrock / Copilot / Cloudflare 等后置。
- **TUI 不用 ratatui**，坚持 pi 的字符串组件 + 差分输出模型。
- **工作树纪律**：不回退/覆盖他人改动；`pi/` 与 `pi-rust/` 是两个独立 git 仓库，分别操作。

---

## 6. 风险

- **模型表漂移**：手写模型表易过期，且 DeepSeek 失败测试已暴露不一致——M2 应引入注册表生成机制。
- **TUI 体量大**：`Editor` 等组件单体庞大，M6 需拆分细粒度迭代，避免一次铺太大表面。
- **全局可变注册表**：provider 注册为进程级全局，测试需用唯一 api id 隔离（pi-ai faux 测试已有此模式）。
- **Unicode/终端差异**：宽度与按键协议跨终端不一致，按 spec 用标准库 + 后续补 pi 的边界 case。
- **辅助 crate 无方向**：`pi-mom`/`pi-pods`/`pi-web-ui` 范围未定，贸然投入有返工风险。
