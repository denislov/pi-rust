# pi → Rust 移植路线图 (ROADMAP)

> 状态：M0–M6 完成 · 最后核对：2026-06-19
> 适用范围：`pi-rust/` 工作区（把 TypeScript `pi` monorepo 移植为 Rust）
> 参考基准：`pi/`（上游 TS 实现，行为权威来源）

本文件回答三个问题：**已经实现了什么**、**还差什么**、**接下来按什么顺序做**。
它是对 `docs/superpowers/specs|plans` 各阶段设计文档的上层汇总，用于指导后续工作。

---

## 1. 总览

`pi` 是一个编码 agent harness，TS 侧约 ~98K 行源码（不含生成代码与测试），由四个核心包组成。
Rust 侧已完成 **M0–M6 的核心实现**——headless 编码 agent 可用、交互式 TUI 基本可用、
配色 + Markdown 渲染 polish 已落地。下一步聚焦 TUI-8 剩余 polish 与 M7 周边能力。

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

| 包 / crate | TS 源码 | Rust 源码 | 测试 | 粗略覆盖 | 状态 |
|---|---:|---:|---:|---:|---|
| `pi-ai` | 30,553 | 23,669 | ~80 | ~75% | ✅ M2 完成 |
| `pi-agent-core` | 8,067 | 4,351 | ~60 | ~55% | ✅ M4 完成 |
| `pi-coding-agent` | 48,013 | 6,406 | ~270 | ~15% | ✅ M5+M6 完成 |
| `pi-tui` | 11,696 | 4,277 | ~90 | ~40% | ✅ M6 完成 + TUI-8 起步 |
| `pi-mom` | — | 14 | 1 | — | ⬜ 空壳 |
| `pi-pods` | — | 14 | 1 | — | ⬜ 空壳 |
| `pi-web-ui` | — | 14 | 1 | — | ⬜ 空壳 |
| **合计** | **~98K** | **~38.7K** | **511** | **~40%** | — |

> "TS 源码"为 `src/**/*.ts`（去除 `*.test.ts`）行数；`pi-ai` 含 ~18.7K 行生成的模型注册表。
> 测试数为 `cargo test --workspace` 实际通过数（2026-06-19 核实）。

### 1.3 当前健康度

- 工作区可编译通过（Rust edition 2024）。
- `cargo fmt --check`、`cargo test --workspace`、`cargo check --workspace` **全绿**（511 测试通过）。
- 各阶段 `plans/*.md` 的复选框从未勾选（全为 `[ ]`），**不能**作为完成信号；以实际代码与 git 提交为准。

---

## 2. 各 crate 现状

### 2.1 `pi-ai` — 多 provider LLM 统一 API（✅ M2 完成）

**已实现**
- 线缆兼容的核心类型 + serde 桥接到 pi 的精确 JSON。
- 惯用法流式模型：`EventStream`、`complete()`、`CancellationToken` 取消。
- Provider trait + 按 `api` 键的全局注册表。
- **5 个 provider**：Anthropic、DeepSeek、OpenAI Completions、OpenAI Responses、Google GenAI。
- **生成式模型注册表**（`models_generated.rs`，~18.7K 行，从 TS 元数据生成）。
- 客户端重试 / 超时（`http_retry.rs`）；env-key 解析（`env_keys.rs`）。
- faux provider（离线端到端测试）。
- 流式 JSON 修复 + 部分解析。

**待实现**
- ❌ 其它 provider API：AWS Bedrock、Mistral、Azure OpenAI、Codex Responses。
- ❌ OAuth / Claude-Code 身份 / GitHub Copilot / Cloudflare 网关 / Bedrock SigV4 认证。
- ❌ provider 兼容标志矩阵（Fireworks / z.ai / OpenRouter / Vercel AI Gateway / Qwen 等）。
- ❌ 图像*生成* API；结构化诊断（diagnostics）、session affinity 头。

### 2.2 `pi-agent-core` — agent 运行时（✅ M4 完成）

**已实现**
- `Agent`（`Arc<RwLock<AgentState>>`）：`new` / `add_tool` / `add_message` / `messages` / `prompt` / `abort`。
- `AgentMessage`、`AgentTool`、`AgentConfig`、`AgentEvent`。
- `run_loop`：顺序工具调用循环、`max_turns`、取消传播、按 stop-reason 分支。
- **compaction**（上下文压缩 + token 计量：`compaction/` 6 文件）。
- **会话持久化**（`session/` 8 文件：JSONL、内存存储、分支树、session repo）。
- **钩子**（`hooks.rs`：`beforeToolCall` / `afterToolCall` 等）。
- steering / follow-up 消息队列（`queues.rs`）。
- `convert_to_context`；测试用 `ApiProvider`。

**待实现**
- ❌ 并行工具执行 + `ToolExecutionMode`。
- ❌ skills 加载、prompt templates（参数替换）。
- ❌ 自定义消息类型（BashExecution / Custom / BranchSummary / CompactionSummary）。
- ❌ thinking level、proxy 流式、`FileSystem`/`Shell` 抽象、类型化错误类。

### 2.3 `pi-coding-agent` — 编码 agent CLI（✅ M5+M6 完成）

**已实现**
- library-first 结构：`run_cli()`；瘦 `main.rs`。
- 参数解析：`-p`/`--print`、`--model`、`--api-key`、`--system-prompt`、`--max-turns`、`--mode`、`--session`/`--continue`/`--resume`/`--fork`、`--thinking` 等。
- **7 个内置工具**：`read` / `write` / `edit` / `bash` / `grep` / `find` / `ls`（含输出截断、cwd、diff）。
- **print 模式**（`-p`）：跑一轮 prompt 输出最终 assistant 文本。
- **headless 协议模式**：`--mode json` 事件流 + RPC（stdio JSON-RPC）。
- **会话管理**：JSONL 持久化、continue/resume/fork/clone/tree、session id、cwd 关联。
- **交互式 TUI 模式**：transcript 布局/滚动、editor、footer、Ctrl+O 工具展开、Ctrl+C 中止、welcome 行、usage 统计。
- runtime：`lookup_model`、`register_builtins`、构建 `AgentConfig`。

**待实现**
- ❌ settings 管理（全局 + 项目级合并）、auth 存储（auth.json / OAuth / 20+ provider env）。
- ❌ 扩展系统（loader / hooks / 自定义工具 / slash 命令 / UI / keybindings）、21 个内置 slash 命令。
- ❌ skills / prompt templates / themes / 资源加载、上下文文件发现（AGENTS.md/CLAUDE.md）。
- ❌ HTML 导出 / 分享、keybindings 配置、迁移逻辑。
- ❌ `@file`、图像输入、stdin 管道、`--models` 模型轮换。

### 2.4 `pi-tui` — 终端 UI（✅ M6 完成 + TUI-8 起步）

**已实现**
- `Component` trait + `Container`、`Text` / `Spacer` / `Input` / `Editor` / `SelectList` / `Markdown`。
- `Tui<T: Terminal>` 渲染管理器：**内联 owned-region 差分渲染**（`RenderSurface::Inline` 默认）、
  全量重绘、`LINE_RESET` 每行复位、`LineTooWide` 校验。
- `Terminal` trait + `ProcessTerminal`（crossterm）、`VirtualTerminal`（测试后端，追踪 cursor/clear 状态）。
- **输入栈**：raw mode、stdin 缓冲、bracketed paste、Kitty 键盘协议、按键解析、keybindings。
- **`RenderScheduler`**：request/force/coalescing/限速（~60Hz）。
- **光标稳定性**：`CURSOR_MARKER` + `position_hardware_cursor` flush。
- **transcript 布局/滚动**：viewport、scroll offset、page up/down、"new output below" 指示器。
- **生命周期**：RAII terminal session guard、raw mode 恢复、Ctrl+C 三路径。
- **TUI-8 起步（配色 + Markdown）**：`Style`/`paint`/`paint_with`/`color_enabled` + NO_COLOR/TERM=dumb；
  Markdown 标题加粗、行内 code reverse、代码块 dim 围栏 + dim 内容、引用 dim、规则 dim；
  transcript 按角色着色（user/tool/error/system/footer）。
- 宽度工具：ANSI-aware `visible_width` / `truncate_to_width`。

**待实现（TUI-8 剩余 + TUI-7）**
- ❌ **spinner/progress**：运行中 agent/tool 的动画指示。
- ❌ **SelectList 菜单**：model/session/status 切换。
- ❌ **主题系统**：256 色/dark/light/custom、能力探测。
- ❌ **TUI-7 跨终端 smoke 套件**：tmux 脚本 + 终端行为记录表。
- ❌ 硬件光标 / IME marker 边界 case；终端图像协议（Kitty/iTerm2）。
- ❌ emoji 探测 / 宽度缓存 / 高级换行。

### 2.5 `pi-mom` / `pi-pods` / `pi-web-ui` — 范围未定义

- 三者均为 14 行 `cargo new` 桩，无设计文档，不在 AGENTS.md 的四包迁移图内。
- **行动项**：在投入前先明确它们对应 TS 侧的什么（或上游新需求），否则保持空壳。

---

## 3. 路线图（里程碑）

M0–M6 已完成。下一阶段聚焦 **TUI-8 剩余 polish** 与 **M7 周边能力**。

### ✅ M0 — 稳定化（已完成）
- 修复 `lookup_deepseek_model` 失败；工作区测试全绿。

### ✅ M1 — 内置工具集（已完成）
- 7 个工具（read/write/edit/bash/grep/find/ls）实现并接入 CLI。

### ✅ M2 — Provider 广度 + 模型注册表（已完成）
- OpenAI Completions/Responses、Google GenAI、生成式模型注册表、env-key 扩展、重试/超时。

### ✅ M3 — 会话持久化（已完成）
- JSONL/内存存储、分支树、continue/resume/fork/clone、session id、cwd 关联。

### ✅ M4 — agent-core harness 能力（已完成）
- compaction、钩子、steering/follow-up 队列、session 层。

### ✅ M5 — headless 协议模式（已完成）
- `--mode json` 事件流 + RPC（stdio JSON-RPC）。

### ✅ M6 — 交互式 TUI（已完成）
- 输入栈、`RenderScheduler`、光标稳定性、transcript 布局/滚动、生命周期、内联渲染。

### 🟡 TUI-8 — 交互 polish（进行中）
- ✅ 配色 + Markdown（8 色语义化 + Markdown 标题/行内 code/代码块/引用/规则样式 + transcript 角色着色）
- ❌ spinner/progress（运行中动画指示）
- ❌ SelectList 菜单（model/session/status 切换）
- ❌ 主题系统（256 色/dark/light/custom + 能力探测）

### ⬜ TUI-7 — 跨终端 smoke 套件
- tmux 脚本 + 终端行为记录表（wezterm/kitty/iTerm2/Terminal.app/GNOME Terminal/tmux/SSH）。
- 作为交互模式可用的发布门。

### ⬜ M7 — 周边能力
- 扩展系统（loader + hooks + 自定义工具 + slash 命令 + UI/keybindings）、auth 存储 / OAuth、
  settings 管理、themes、HTML 导出 / 分享、资源加载、迁移逻辑、剩余 provider 与兼容矩阵。

### 横切项 — 辅助 crate 定位
- 为 `pi-mom` / `pi-pods` / `pi-web-ui` 明确目标范围（或决定暂缓 / 移除）。

---

## 4. 建议的近期下一步

1. **TUI-8 spinner/progress**：为运行中 agent/tool 加动画指示，延续刚完成的配色工作线。
2. **TUI-8 SelectList 菜单**：model/session/status 切换。
3. **TUI-7 smoke 套件**：固化跨终端行为，作为交互模式发布门。
4. **M7 周边能力**：按需切片（扩展系统、auth/settings 等）。

---

## 5. 关键决策与约束（沿用各 spec）

- **离线优先**：所有测试不依赖真实 provider key；用 faux provider / fixture / 单元测试证明正确性。
- **线缆兼容**：事件协议与 wire JSON 与 pi 保持字节级一致（serde 桥接）。
- **惯用 Rust**：snake_case + enum + `Result`/typed error。
- **小 crate 对应 TS 包边界**，不向根 package 堆叠跨切面代码。
- **认证先只做 API-key 路径**；OAuth / Bedrock / Copilot / Cloudflare 等后置。
- **TUI 不用 ratatui**，坚持 pi 的字符串组件 + 差分输出模型。
- **TUI-8 配色**：8 色语义化，NO_COLOR + TERM=dumb 禁用，`paint_with` 供测试显式控制。
- **工作树纪律**：不回退/覆盖他人改动；`pi/` 与 `pi-rust/` 是两个独立 git 仓库，分别操作。

---

## 6. 风险

- **模型表漂移**：生成式注册表已落地，但 TS 上游模型更新时需重新生成。
- **TUI-8 主题系统体量大**：256 色/能力探测/跨终端一致性测试，需拆分细粒度迭代。
- **TUI-7 终端差异**：宽度与按键协议跨终端不一致，smoke 套件需覆盖主流终端。
- **M7 范围大**：扩展系统/auth/settings/导出各自是独立子系统，需逐个 spec→plan→实现。
- **全局可变注册表**：provider 注册为进程级全局，测试需用唯一 api id 隔离。
- **辅助 crate 无方向**：`pi-mom`/`pi-pods`/`pi-web-ui` 范围未定，贸然投入有返工风险。
