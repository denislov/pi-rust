# pi-coding-agent 工程质量优化方案

本文档面向 `pi-rust/crates/pi-coding-agent` 的后续重构和维护。目标是在保持 TypeScript `pi/packages/coding-agent` 行为兼容、现有 Rust 测试通过、工具继续具备自由读写/执行能力的前提下，把当前迁移期实现优化为稳定 library API、边界清晰、RPC 可并发流式处理、交互层可维护、工具层可持续扩展的 Rust crate。

结论先行：`pi-coding-agent` 当前不是低质量代码。它已经有 CLI、print/json/rpc、interactive TUI、session persistence、resources、built-in tools、配置认证和大量离线测试。真正的问题是功能推进速度快于架构收敛，复杂度集中在少数长文件和重复装配路径中。最应优先处理的是 RPC 控制流；其次是稳定 public API、interactive 拆分和共享 prompt context；最后是工具层重复、参数类型化和运行选项表达清理。

## 已确认约束

- `pi-coding-agent` 需要作为稳定的 library API 被其他 crate 依赖。
- 内置工具继续拥有自由读写/执行能力，不引入默认 workspace sandbox 或审批限制。
- 可以引入统一 capability 抽象，但它的默认策略必须是 allow-all，用于一致性、审计、测试替身和未来扩展，而不是改变现有自由能力。
- TUI 目前不依赖 RPC，也不应为了这次优化改成依赖 RPC。
- RPC 需要重构，目标是支持真正的异步事件流、running 状态命令、abort、steer、follow-up，而不是当前 prompt 完成后批量输出事件的模式。
- 不修改 `pi/` TypeScript reference repo。
- 不要求真实 provider key 参与测试。

## 当前状态概览

当前 crate 主要模块：

```text
crates/pi-coding-agent/src/
  args.rs
  config/
  input.rs
  interactive/
  protocol/
  resources.rs
  runtime.rs
  session.rs
  tools/
  lib.rs
  main.rs
```

已有优点：

- `protocol/session_runner.rs` 已经统一了 print/json/interactive 的核心 agent 驱动路径。
- `interactive/event_bridge.rs` 和 `interactive/transcript.rs` 已经把一部分 UI event 和 transcript 状态拆出。
- 多数文件工具提供 `*Operations` trait，例如 `ReadOperations`、`WriteOperations`、`EditOperations`、`BashOperations`，便于测试替身。
- `file_mutation_queue` 已经处理同文件并发写入顺序。
- 测试覆盖较广：CLI、args、runtime、config、session、interactive、RPC、JSONL、tools、resources 都有离线测试。

主要结构压力：

- `protocol/rpc.rs` 同时负责 JSONL IO、command parsing、RPC state、prompt 执行、队列、stats、event buffering，且 prompt 执行期间不能继续读取命令。
- `interactive/app.rs` 超过 4700 行，混合 TUI state、slash command、render、event loop、session import/export、clipboard、prompt context 和 test harness。
- `lib.rs::run_cli_with_options_and_stdin` 与 `interactive/app.rs::build_prompt_context` 重复配置/资源/auth/session/tool 装配逻辑。
- `tools/{ls,find,grep}.rs` 重复 limit 解析、路径显示、walk/filter、排序和输出 notice 逻辑。
- `lib.rs` 暴露面过宽，`interactive::test_harness` 在普通 build 中公开，不适合稳定 library API。
- `SessionPromptOptions`、`PrintModeOptions`、`PromptInvocation` 的语义有重复字段和无效字段。

本方案把这些问题拆成 P1、P2、P3 三组推进。

## 推荐总体路线

推荐采用 strangler-style incremental refactor，而不是一次性重写：

1. P1 先修 RPC 控制流。RPC 是当前最明确的行为缺陷，且可独立于 TUI 完成。
2. P2 建立稳定 library API facade，再拆 interactive 和共享 prompt context。先稳定外部依赖边界，再重排内部文件。
3. P3 收敛工具层重复、参数类型和运行选项表达，减少未来维护成本。

每个阶段都必须保持：

```bash
cargo fmt --check
cargo test -p pi-coding-agent
cargo check -p pi-coding-agent
```

跨 crate API 改动后再运行：

```bash
cargo test --workspace
cargo check --workspace
```

## P1：RPC 控制流重构

### 问题

当前 `protocol/rpc.rs` 的 `run_rpc_mode_for_io` 按顺序读取一行命令并等待 `state.handle_command` 完成。`handle_prompt` 在接受 prompt 后等待 `run_session_prompt` 返回，期间不会继续读取 stdin；agent events 被先放入 `event_lines`，等 prompt 完成后再写回 stdout。

这带来几个直接问题：

- `abort` 命令无法在 prompt 运行期间被读取。
- `steer` / `follow_up` / `prompt` with `streamingBehavior` 不能真正作用于正在运行的 agent。
- `is_streaming` 是局部状态标记，但命令循环被阻塞后无法体现长连接 RPC 状态机价值。
- RPC 输出不是真正流式事件，客户端只能在 prompt 结束后看到事件批量输出。
- `rpc.rs` 文件过大，command parsing、runtime state、prompt task、stats、event adapter 全耦合。

### 目标

P1 完成后，RPC 应满足：

- RPC 进程在 prompt 运行时继续读取 stdin 命令。
- `prompt` 接受后立即返回 response，并开始异步输出 `agent_start`、`turn_start`、message/tool events。
- `abort` 在 running 状态调用当前 agent 的 abort handle，并保持 RPC 进程存活。
- `steer` 和 `follow_up` 在 running 状态进入当前 agent 队列，并发出 `queue_update`。
- running 状态下新的 `prompt` 如果没有 `streamingBehavior`，返回结构化错误；如果为 `steer` 或 `followUp`，转发到对应队列。
- agent 完成、失败或被 abort 后，RPC state 恢复 idle，并更新 messages、session path、leaf id、stats。
- TUI 不依赖 RPC。RPC 重构不得改变 interactive 的 prompt loop 架构。

### 目标模块结构

建议把当前单文件 `protocol/rpc.rs` 迁移成目录模块：

```text
crates/pi-coding-agent/src/protocol/rpc/
  mod.rs
  io.rs
  state.rs
  commands.rs
  prompt_task.rs
  stats.rs
```

职责：

- `mod.rs`：保留稳定入口 `run_rpc_mode_stdio`、`run_rpc_mode_for_io`、`write_rpc_response`，对外 re-export 必要类型。
- `io.rs`：JSONL read/write helper，包装现有 `JsonlLineReader` 和 `serialize_json_line`。
- `state.rs`：`RpcState`、session/model/auth/settings、queue state、current prompt task。
- `commands.rs`：command validation、command dispatch、unsupported command mapping。
- `prompt_task.rs`：`RunningPrompt`、spawn、event polling、done finalization。
- `stats.rs`：`session_state`、`session_stats`、`last_assistant_text`。

如果不想一次移动文件，可先在 `rpc.rs` 内部创建私有 structs/functions，测试稳定后再机械拆分为目录模块。

### 核心设计

当前 `SessionPromptAbortHandle` 只暴露 `abort()`。因为它内部已经持有 `pi_agent_core::Agent`，建议扩展为更完整的控制 handle：

```rust
pub struct SessionPromptControlHandle {
    agent: Agent,
}

impl SessionPromptControlHandle {
    pub fn abort(&self) {
        self.agent.abort();
    }

    pub fn steer(&self, text: impl Into<String>) {
        self.agent.steer(text);
    }

    pub fn follow_up(&self, text: impl Into<String>) {
        self.agent.follow_up(text);
    }
}
```

兼容策略：

- 保留 `SessionPromptAbortHandle` type alias 或 wrapper，避免现有 interactive 调用方被迫同步修改。
- `SpawnedSessionPrompt` 可以新增 `control` 字段，也可以把 `abort` 字段类型扩展为具备 `steer/follow_up` 的 handle。

RPC prompt task 状态：

```rust
struct RunningPrompt {
    control: SessionPromptControlHandle,
    events: mpsc::UnboundedReceiver<AgentEvent>,
    done: oneshot::Receiver<Result<SessionPromptResult, CliError>>,
    adapter: ProtocolEventAdapter,
    abort_requested: bool,
}
```

主循环从“读一条命令处理完”改为 select loop：

```text
loop:
  select:
    stdin line arrives:
      parse command
      if running: handle running-state command
      if idle: handle idle-state command

    agent event arrives from running prompt:
      adapter.push(event)
      write protocol events immediately

    running prompt done:
      drain remaining events
      update messages/session/leaf/stats
      clear running
```

这与 interactive 现有 async loop 思路一致，但 RPC 只面向 JSONL protocol，不引入 TUI 组件依赖。

### Command 行为

`prompt`：

- idle 状态：写 success response 后创建 `RunningPrompt`，立即写 `agent_start` event，然后进入 running。
- running 且 `streamingBehavior == steer`：调用 `control.steer(message)`，写 success response，发 `queue_update`。
- running 且 `streamingBehavior == followUp`：调用 `control.follow_up(message)`，写 success response，发 `queue_update`。
- running 且没有 streaming behavior：写 error response。

`steer`：

- running：调用 `control.steer(message)`，写 success response，发 `queue_update`。
- idle：保留现有语义可放入 pending queue，也可以返回 error；推荐保持现有“成功入队”行为，然后下一次 prompt 前合并，避免破坏 M5 兼容。

`follow_up`：

- running：调用 `control.follow_up(message)`，写 success response，发 `queue_update`。
- idle：保持现有 pending queue 行为。

`abort`：

- running：调用 `control.abort()`，设置 `abort_requested = true`，写 success response。最终错误/aborted event 由 agent stream 产生。
- idle：写 success response，`cancelled: false` 或空 data，保持幂等。

`new_session`：

- running：应先拒绝，返回 `"cannot start new session while agent is streaming"`。不要隐式 abort，避免客户端误以为 prompt 已安全保存。
- idle：清空 messages、queues、session name、session target，并返回 success。

### Session 和 stats

当前 RPC `session_state` 返回 `"in-memory"` 和 `session_file: None`，但 `run_session_prompt` 实际可以写 session。P1 应把 `SessionPromptResult.session_path` 和 `leaf_id` 保存在 `RpcState` 中：

```rust
struct RpcState {
    active_session_path: Option<PathBuf>,
    active_leaf_id: Option<String>,
    messages: Vec<StoredAgentMessage>,
    running: Option<RunningPrompt>,
    steering: Vec<String>,
    follow_up: Vec<String>,
    session_name: Option<String>,
}
```

`get_state`：

- `is_streaming` 来自 `running.is_some()`。
- `session_file` 返回 active session path。
- `session_id` 可从 session header 或 path metadata 解析；解析失败时回退 `"in-memory"`。

`get_session_stats`：

- 第一阶段可以继续使用 message counts 和 zero token/cost。
- 后续可从 assistant `StoredUsage` 聚合 token/cost。

### 测试计划

新增或修改测试：

- `tests/rpc_mode.rs::rpc_streams_events_before_prompt_done`
  - faux provider 分段输出；
  - prompt response 后能读到 `agent_start` 和 text delta，而不是等待 stdin EOF。
- `tests/rpc_mode.rs::rpc_abort_cancels_running_prompt`
  - provider 等待 cancel token；
  - 发送 prompt 后发送 abort；
  - 断言 provider 收到 cancel，RPC 输出 success response 和 error/agent_end。
- `tests/rpc_mode.rs::rpc_steer_while_running_updates_queue`
  - 发送 prompt 后发送 steer；
  - 断言返回 success 和 `queue_update`。
- `tests/rpc_mode.rs::rpc_follow_up_while_running_updates_queue`
  - 同 steer。
- `tests/rpc_mode.rs::rpc_prompt_while_running_requires_streaming_behavior`
  - running 时发送普通 prompt；
  - 断言结构化 error。
- `tests/protocol_sessions.rs::rpc_state_reports_persisted_session_path_after_prompt`
  - prompt 完成后 `get_state` 返回 session file。

验收命令：

```bash
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test protocol_sessions
cargo test -p pi-coding-agent
```

### 风险与缓解

- 风险：`tokio::select!` 中持有 mutable writer、reader、running receiver 的 borrow 关系变复杂。  
  缓解：把 `RunningPrompt` 从 state 中 `take()` 出来处理，事件处理后再放回，或为 `RunningPrompt` 提供 `poll_next_event` helper。

- 风险：事件顺序与现有 JSON mode 不一致。  
  缓解：继续使用 `ProtocolEventAdapter`，只改变输出时机，不改变 event shape。

- 风险：abort 后 session capture 行为变化。  
  缓解：保持 `session_runner` 是唯一 capture 路径，RPC 不直接写 session。

## P2：稳定 library API 与 interactive/context 模块化

P2 包含三件相关工作：

1. 明确并稳定 public API。
2. 把 CLI/interactive 的 shared prompt context 装配收敛到一个 builder。
3. 拆分 `interactive/app.rs`，降低维护成本。

这三件事应按顺序做。先稳定外部 API，再重排内部实现，避免其他 crate 依赖到不该依赖的内部模块。

### P2-A：稳定 public API

#### 问题

当前 `lib.rs` 暴露了几乎所有模块：

```rust
pub mod args;
pub mod config;
pub mod error;
pub mod input;
pub mod interactive;
pub mod models;
pub mod print_mode;
pub mod protocol;
pub mod resources;
pub mod runtime;
pub mod session;
pub mod tools;
```

这对迁移期方便，但对稳定 library API 不合适。其他 crate 一旦依赖这些内部模块，后续拆分 `interactive/app.rs`、`protocol/rpc.rs`、`resources.rs` 都会变成破坏性 API 改动。

#### API 分层

建议引入三层 API 约定：

1. Stable API：承诺 semver 兼容，其他 crate 推荐使用。
2. Incubating API：可用但可能调整，必须通过 feature 或 module docs 标注。
3. Internal API：crate 内部实现，不鼓励外部依赖。

建议稳定入口：

```rust
pub mod api {
    pub use crate::args::{CliArgs, CliMode, help_text, parse_args};
    pub use crate::error::CliError;
    pub use crate::runtime::{
        CliRunOptions,
        SessionMode,
        SessionRunOptions,
        DEFAULT_MODEL_ID,
        DEFAULT_SYSTEM_PROMPT,
    };
    pub use crate::{CliOutput, run_cli, run_cli_with_options, run_cli_with_options_and_stdin};
    pub use crate::session::{ResolvedSessionTarget, ActiveSession};
    pub use crate::protocol::session_runner::{
        PromptInvocation,
        SessionPromptOptions,
        SessionPromptResult,
        run_session_prompt,
        spawn_session_prompt,
    };
    pub use crate::tools::{ToolFilter, builtin_tools, filter_tools};
}
```

兼容策略：

- 第一阶段保留现有 root-level re-export，不立即删除。
- 为 module-level public API 增加 docs，标注哪些稳定、哪些 internal。
- `interactive::test_harness` 不应默认公开。建议改为：

```rust
#[cfg(any(test, feature = "test-harness"))]
pub mod test_harness;
```

并在 `Cargo.toml` 增加：

```toml
[features]
default = []
test-harness = []
```

如果现有 integration tests 需要该 harness，测试命令可以继续通过 crate 内部 `#[cfg(test)]` 或 dev feature 使用。对外 crate 如确需脚本化 TUI 测试，可以显式启用 `test-harness` feature。

#### 稳定性规则

新增 `docs` 或 crate-level docs 说明：

- Stable API 的结构体字段新增需优先使用 builder 或 `#[non_exhaustive]`。
- Error enum `CliError` 新增 variant 需要评估调用方 pattern match 影响；可以加 `#[non_exhaustive]`。
- `SessionPromptOptions` 这类跨模式选项应尽快迁移到 builder，避免每次新增字段都破坏构造代码。
- `protocol::types` wire structs 是否稳定要单独声明。推荐把 JSON/RPC wire shape 视为 protocol contract，但 Rust struct module 先标为 incubating。

### P2-B：共享 prompt context builder

#### 问题

headless 路径在 `lib.rs::run_cli_with_options_and_stdin` 中完成配置加载、model selection、auth、resources、context files、tool filter、session target、`PromptInvocation` 构建。interactive 在 `interactive/app.rs::build_prompt_context` 又做一套近似逻辑。

这会导致：

- print/json/interactive 对 config、resources、context files 的行为容易分叉。
- 新增 CLI flag 时要改两套逻辑。
- stable library API 调用方难以复用“解析好的 prompt request”。

#### 目标模块

新增：

```text
crates/pi-coding-agent/src/request/
  mod.rs
  builder.rs
  context.rs
  invocation.rs
```

或更保守地先新增单文件：

```text
crates/pi-coding-agent/src/request.rs
```

核心类型：

```rust
pub struct ResolvedCliContext {
    pub cwd: PathBuf,
    pub parsed: CliArgs,
    pub config: config::Config,
    pub config_paths: config::ConfigPaths,
    pub model: Model,
    pub api_key: Option<String>,
    pub resources: AgentResources,
    pub loaded_skills: Vec<Skill>,
    pub loaded_prompt_templates: Vec<PromptTemplate>,
    pub loaded_themes: Vec<ThemeResource>,
    pub selected_theme: Option<ThemeResource>,
    pub system_prompt: Option<String>,
    pub tools: Vec<AgentTool>,
    pub session: Option<SessionRunOptions>,
    pub session_target: Option<ResolvedSessionTarget>,
    pub session_name: Option<String>,
    pub thinking_level: Option<ThinkingLevel>,
    pub tool_execution: Option<ToolExecutionMode>,
}

pub struct ResolvedPromptRequest {
    pub session_options: SessionPromptOptions,
    pub processed_prompt: ProcessedPromptInput,
}
```

Builder 输入：

```rust
pub struct ResolveCliRequestOptions {
    pub cli_options: CliRunOptions,
    pub stdin: Option<String>,
    pub require_prompt: bool,
    pub mode: ResolveMode,
}
```

`ResolveMode` 用于处理 headless 和 interactive 的差异：

- headless print/json：需要 prompt 或 stdin。
- interactive：不需要立即 prompt，但需要 model/auth/resources/session/theme/model choices。
- rpc：长连接启动时加载 config/model/auth/tools，单个 prompt command 再构造 `SessionPromptOptions`。

#### 行为约束

- `resources::discover_context_files` 只在 builder 中调用一次。
- `config::load_config` 和 `config::auth::resolve_api_key` 的 diagnostics 通过 `Vec<Diagnostic>` 返回，不在 builder 内直接 `eprint!`。CLI entrypoint 决定输出到 stderr，library 调用方可以选择记录或忽略。
- `tools::filter_tools` 在 builder 中完成。
- `PromptInvocation` 由 builder 根据 skill/template/content/text 统一生成。
- interactive 的 model choices、session choices、theme 可在 `InteractivePromptContext` 中从 `ResolvedCliContext` 派生，避免污染 headless request。

### P2-C：拆分 interactive/app.rs

#### 问题

`interactive/app.rs` 同时负责：

- `InputPump`
- `PromptContext`
- slash command registry 和 command handlers
- clipboard
- session import/export/clone
- `InteractiveRoot` state
- render helper
- `Component` impl
- async event loop
- prompt task
- prompt context 构建
- session choice/model choice
- tests
- test harness

长期维护时，任何一类 UI 改动都会触碰大文件，review 成本高，冲突概率高。

#### 目标结构

建议拆成：

```text
crates/pi-coding-agent/src/interactive/
  mod.rs
  app.rs
  root.rs
  render.rs
  loop.rs
  input.rs
  prompt_task.rs
  prompt_context.rs
  slash.rs
  commands.rs
  session_actions.rs
  clipboard.rs
  model_selector.rs
  session_selector.rs
  event_bridge.rs
  transcript.rs
  key_hints.rs
  harness.rs
```

职责：

- `app.rs`：仅保留 `run_interactive_mode` 和 top-level wiring。
- `root.rs`：`InteractiveRoot` state、`Component` impl、基础 input dispatch。
- `render.rs`：footer、transcript rows、tool rows、editor border、fit_line。
- `loop.rs`：`run_interactive_loop`、`run_started_interactive_loop`、render scheduler。
- `input.rs`：`InputPump` 和 stdin chunk source。
- `prompt_task.rs`：`PromptTask`、spawn、abort-once。
- `prompt_context.rs`：interactive-specific context derived from shared request builder。
- `slash.rs`：slash command registry、parser、completion。
- `commands.rs`：slash command handlers that mutate root or return action.
- `session_actions.rs`：import/export/clone/session choice collection。
- `clipboard.rs`：`ClipboardSink` 和 platform copy implementation。
- `model_selector.rs` / `session_selector.rs`：selection/filter/render/input helpers。
- `harness.rs`：scripted interactive test harness，feature gated。

拆分顺序：

1. 先移动 `InputPump` 到 `input.rs`，无行为变化。
2. 移动 `ClipboardSink`、`SystemClipboard`、copy helpers 到 `clipboard.rs`。
3. 移动 `SessionChoice`、import/export/clone helpers 到 `session_actions.rs`。
4. 移动 slash command registry/parser/completion 到 `slash.rs`。
5. 移动 render helper 到 `render.rs`。
6. 移动 async loop 和 `PromptTask` 到 `loop.rs` / `prompt_task.rs`。
7. 最后移动 test harness 到 `harness.rs` 并加 feature gate。

每一步都应是机械迁移加 `cargo test -p pi-coding-agent --test interactive_mode`，避免在拆分中顺手改行为。

### P2 验收

P2 完成后：

- 外部 crate 有明确稳定入口 `pi_coding_agent::api::*`。
- `interactive::test_harness` 不在默认 public API 中出现。
- `run_cli_with_options_and_stdin` 和 interactive context 使用同一 shared builder。
- `interactive/app.rs` 降到 300 行以内，只负责 top-level wiring。
- `interactive/root.rs` 可偏大，但不应包含 session import/export、clipboard platform command、test harness。
- 所有现有 interactive 测试保持通过。

验收命令：

```bash
cargo test -p pi-coding-agent --test cli
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_sessions
cargo test -p pi-coding-agent
cargo check --workspace
```

## P3：工具层、运行选项与维护性收敛

P3 不应改变用户可见能力。它的重点是减少重复、增强类型表达、保持稳定 API 可演进。

### P3-A：工具 capability 抽象，但默认 allow-all

#### 问题

工具目前都直接调用 `resolve_to_cwd` 和 `tokio::fs` / `Command`。路径解析允许绝对路径和 `~`，这符合“自由读写/执行能力”的产品约束。但所有工具各自处理路径、错误和 IO，缺少统一能力入口。

#### 目标

新增内部能力抽象：

```text
crates/pi-coding-agent/src/tools/capability.rs
```

核心类型：

```rust
pub trait ToolCapability: Send + Sync {
    fn resolve_path(&self, raw: &str, cwd: &Path) -> Result<PathBuf, String>;
    fn describe_path_policy(&self) -> &'static str;
}

pub struct AllowAllToolCapability;
```

默认策略：

- `AllowAllToolCapability` 行为等价于当前 `resolve_to_cwd`。
- 允许绝对路径。
- 允许 `~`。
- 不拒绝 cwd 外路径。
- 不审批 bash command。

为什么仍然需要该抽象：

- 保持所有工具路径解析一致。
- 让测试可以注入 deterministic path policy。
- 未来如需审计、日志、dry-run、workspace policy，可在不重写每个工具的情况下接入。
- library 调用方可显式选择自定义 policy，但默认仍为自由能力。

### P3-B：工具参数类型化

#### 问题

多数工具直接从 `serde_json::Value` 手工取字段，重复并且不利于稳定错误语义。例如 `read`、`write`、`edit`、`grep`、`find`、`ls` 都有各自的字段解析方式。

#### 目标

为每个工具新增 args struct：

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadArgs {
    path: String,
    offset: Option<u64>,
    limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrepArgs {
    pattern: String,
    path: Option<String>,
    glob: Option<String>,
    ignore_case: Option<bool>,
    literal: Option<bool>,
    context: Option<u64>,
    limit: Option<u64>,
}
```

兼容要求：

- `edit` 必须继续支持 legacy single-edit args `oldText` / `newText`。
- `edit` 必须继续支持 `edits` 为 JSON string 的兼容输入。
- 数字字段继续接受 integer；是否接受 float 应按当前测试保持，不要无意收紧。
- 错误消息可以更一致，但不能破坏现有关键测试断言。

建议新增工具共享 helper：

```text
crates/pi-coding-agent/src/tools/common.rs
```

包含：

- `text_block`
- `parse_positive_limit`
- `parse_non_negative_context`
- `sort_case_insensitive`
- `relative_posix`
- `basename`
- `format_limit_notice`

### P3-C：walk/search 共享实现

#### 问题

`find.rs` 和 `grep.rs` 都实现了：

- `GlobBuilder`
- skip `.git` / `node_modules`
- `WalkBuilder`
- relative posix display
- basename matching
- case-insensitive sorting

#### 目标

新增：

```text
crates/pi-coding-agent/src/tools/walk.rs
```

提供：

```rust
pub struct WalkOptions {
    pub include_hidden: bool,
    pub respect_gitignore: bool,
    pub skip_common_dirs: bool,
    pub follow_links: bool,
}

pub struct WalkCandidate {
    pub path: PathBuf,
    pub display: String,
    pub basename: String,
    pub is_dir: bool,
}

pub fn collect_candidates(root: &Path, options: &WalkOptions) -> Vec<WalkCandidate>;
```

`find` 和 `grep` 只保留各自 domain logic：

- `find`：pattern target selection、match dir/file、limit/truncation output。
- `grep`：regex/literal matching、context lines、line truncation。

### P3-D：运行选项表达清理

#### 问题

`SessionPromptOptions` 包含 `prompt` 和 `invocation`，但 session runner 实际以 `invocation` 为准；`PrintModeOptions` 复制了 `SessionPromptOptions` 几乎所有字段。

#### 目标

把 prompt 执行输入拆成两类：

```rust
pub struct AgentRunContext {
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub tools: Vec<AgentTool>,
    pub register_builtins: bool,
    pub session: Option<SessionRunOptions>,
    pub session_target: Option<ResolvedSessionTarget>,
    pub session_name: Option<String>,
    pub thinking_level: Option<ThinkingLevel>,
    pub tool_execution: Option<ToolExecutionMode>,
    pub resources: AgentResources,
    pub settings: Option<crate::config::Settings>,
}

pub struct AgentPromptRequest {
    pub invocation: PromptInvocation,
    pub display_prompt: String,
}
```

然后：

```rust
pub struct SessionPromptOptions {
    pub context: AgentRunContext,
    pub request: AgentPromptRequest,
}
```

兼容策略：

- 先新增 builder：`SessionPromptOptions::builder()`。
- 保留旧字段一个迁移周期，内部转换到新结构。
- `PrintModeOptions` 改为 newtype 或直接用 `SessionPromptOptions`，避免重复字段。

### P3-E：错误和 diagnostics 输出边界

#### 问题

library API 不应在深层 builder 或 runtime 中直接 `eprint!` / `eprintln!`。当前 config/resource diagnostics 在 CLI 和 interactive 装配中直接输出。

#### 目标

引入：

```rust
pub struct CliDiagnostic {
    pub severity: DiagnosticSeverity,
    pub source: Option<PathBuf>,
    pub message: String,
}

pub struct ResolvedCliContext {
    pub diagnostics: Vec<CliDiagnostic>,
    pub cwd: PathBuf,
    pub parsed: CliArgs,
    pub model: Model,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub tools: Vec<AgentTool>,
    pub session: Option<SessionRunOptions>,
}
```

CLI binary 决定如何打印 diagnostics。Library 调用方可以选择记录、忽略或转成 UI message。这个改动应与 P2-B shared builder 一起做。

### P3 验收

P3 完成后：

- 工具默认能力与当前一致，仍可读写绝对路径、`~` 路径并执行 bash。
- `read/write/edit/bash` 的 `*Operations` 注入能力继续存在。
- `find/grep/ls` 的重复 helper 明显减少。
- `SessionPromptOptions` 的 prompt 语义清晰，不再有“字段存在但被忽略”的主要路径。
- library API 的深层调用不会直接向 stderr 输出 diagnostics，除非通过 CLI entrypoint。

验收命令：

```bash
cargo test -p pi-coding-agent --test tool_read
cargo test -p pi-coding-agent --test tool_write
cargo test -p pi-coding-agent --test tool_edit
cargo test -p pi-coding-agent --test tool_bash
cargo test -p pi-coding-agent --test tool_find
cargo test -p pi-coding-agent --test tool_grep
cargo test -p pi-coding-agent --test tool_ls
cargo test -p pi-coding-agent --test tool_operations
cargo test -p pi-coding-agent
```

## 分阶段实施计划

### 阶段 0：冻结行为护栏

目的：在重构前增加 characterization tests，避免结构改动改变协议和工具行为。

建议先补：

- RPC running prompt tests：streaming、abort、steer、follow-up。
- Stable API import tests：其他 crate 预期使用的 `pi_coding_agent::api::*`。
- Tool freedom tests：绝对路径、`~`、cwd 外路径、bash execution 继续允许。
- Diagnostics tests：builder 返回 diagnostics，不直接打印。

验收：

```bash
cargo test -p pi-coding-agent
```

### 阶段 1：P1 RPC async loop

目的：修复最明确的行为缺陷。

步骤：

1. 为 `SessionPromptAbortHandle` 增加 steer/follow-up 能力或新增兼容 control handle。
2. 把 `RpcState` 的 running 状态改为 `Option<RunningPrompt>`。
3. 把 `run_rpc_mode_for_io` 改成 stdin/events/done select loop。
4. 让 `ProtocolEventAdapter` 输出事件时机变为实时写出。
5. 保存 prompt result 到 RPC state，用于 `get_state` 和 stats。
6. 拆分 `protocol/rpc.rs`，或至少先按内部 sections 收口。

验收：

```bash
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test protocol_events
cargo test -p pi-coding-agent --test protocol_sessions
cargo test -p pi-coding-agent
```

### 阶段 2：P2 stable API facade

目的：在内部大拆分前给其他 crate 一个明确依赖面。

步骤：

1. 新增 `api` facade。
2. 给稳定类型增加文档和必要的 `#[non_exhaustive]`。
3. 保留旧 re-export，避免立即破坏调用方。
4. feature gate `interactive::test_harness`。
5. 新增 public API import tests。

验收：

```bash
cargo test -p pi-coding-agent --test public_api
cargo check --workspace
```

### 阶段 3：P2 shared request builder

目的：消除 headless 和 interactive 的装配重复。

步骤：

1. 新增 `request` 模块和 `ResolvedCliContext`。
2. 把 config/model/auth/resources/context files/tool filter/session target 装配迁移进去。
3. `run_cli_with_options_and_stdin` 改为使用 builder。
4. interactive `build_prompt_context` 改为从 builder 派生 interactive context。
5. diagnostics 从直接输出改为返回到 entrypoint。

验收：

```bash
cargo test -p pi-coding-agent --test cli
cargo test -p pi-coding-agent --test interactive_args
cargo test -p pi-coding-agent --test m10_resources_input
cargo test -p pi-coding-agent
```

### 阶段 4：P2 interactive 文件拆分

目的：降低交互层维护成本。

步骤：

1. 机械移动 input、clipboard、session actions、slash、render。
2. 移动 event loop 和 prompt task。
3. 移动 selector helper。
4. 移动 harness 并 feature gate。
5. 保持每一步小提交和测试通过。

验收：

```bash
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent --test interactive_abort
cargo test -p pi-coding-agent --test interactive_sessions
cargo test -p pi-coding-agent
```

### 阶段 5：P3 tools/common/capability

目的：减少工具层重复，并保持自由能力。

步骤：

1. 新增 `tools/capability.rs`，默认 `AllowAllToolCapability`。
2. 新增 `tools/common.rs`，迁移 `text_block`、limit/context parser、sort helper。
3. 新增 `tools/walk.rs`，迁移 find/grep 共享 walk 逻辑。
4. 为每个工具引入 typed args，保留兼容输入。
5. 增加 tool freedom tests。

验收：

```bash
cargo test -p pi-coding-agent --test tool_operations
cargo test -p pi-coding-agent --test tools_e2e
cargo test -p pi-coding-agent --test tool_bash
cargo test -p pi-coding-agent --test tool_read
cargo test -p pi-coding-agent --test tool_write
cargo test -p pi-coding-agent --test tool_edit
cargo test -p pi-coding-agent --test tool_find
cargo test -p pi-coding-agent --test tool_grep
cargo test -p pi-coding-agent --test tool_ls
```

### 阶段 6：P3 run options cleanup

目的：让 prompt request 和 run context 语义清晰。

步骤：

1. 新增 `AgentRunContext` 和 `AgentPromptRequest`。
2. 给 `SessionPromptOptions` 增加 builder。
3. 让 print/json/rpc/interactive 都通过 builder 生成 options。
4. 把 `PrintModeOptions` 收敛为 wrapper 或废弃兼容层。
5. 更新 public API docs。

验收：

```bash
cargo test -p pi-coding-agent --test print_mode
cargo test -p pi-coding-agent --test json_mode
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test harness_print_mode
cargo test -p pi-coding-agent
```

## 稳定 API 建议清单

建议其他 crate 只依赖以下入口：

```rust
use pi_coding_agent::api::{
    CliArgs,
    CliMode,
    CliOutput,
    CliRunOptions,
    SessionRunOptions,
    SessionMode,
    SessionPromptOptions,
    SessionPromptResult,
    PromptInvocation,
    parse_args,
    run_cli_with_options_and_stdin,
    run_session_prompt,
    spawn_session_prompt,
    builtin_tools,
};
```

不建议稳定依赖：

- `interactive::app::*`
- `interactive::test_harness`，除非启用 `test-harness` feature。
- `protocol::rpc` 内部 state 类型。
- `resources` 内部 theme parser helper。
- `tools::*_execute_with_operations` 以外的工具内部 helper。

如果已有 crate 需要更多 API，应优先通过 `api` facade 明确导出，而不是继续扩大 `pub mod` 暴露面。

## 非目标

- 不把 `pi-coding-agent` 拆成多个 crate。
- 不让 TUI 改走 RPC。
- 不引入默认 sandbox、审批或权限拦截。
- 不改变 built-in tool 的用户可见名称、schema 或默认能力。
- 不改变 session JSONL v3 wire format。
- 不要求 TypeScript repo 测试运行。
- 不追求一次性删除所有旧 re-export。稳定 API 迁移应逐步完成。

## 风险排序

最高风险：

- RPC async loop 的事件顺序、done drain、writer flush 和 session capture。
- Shared builder 改变 CLI/interactive 对 resources/context files/auth 的细节。

中等风险：

- `interactive/app.rs` 拆分时发生机械移动错误。
- `SessionPromptOptions` builder 迁移时遗漏某个 mode 的字段。

低风险：

- 工具 common helper 提取。
- API facade 增加。
- docs 和 module docs。

## 最终验收标准

功能验收：

- RPC 支持 running 状态下 abort、steer、follow-up，并实时输出 events。
- Stable API facade 存在，并有 public import test。
- TUI 不依赖 RPC，interactive 行为和测试保持不变。
- 工具仍默认自由读写/执行。
- `interactive/app.rs` 和 `protocol/rpc.rs` 复杂度明显下降。

质量验收：

```bash
cargo fmt --check
cargo test -p pi-coding-agent
cargo check -p pi-coding-agent
cargo test --workspace
cargo check --workspace
```

文档验收：

- README 或 crate docs 标注稳定 API 入口。
- `test-harness` feature 的用途说明清楚。
- RPC protocol contract 保持与现有 `protocol/types.rs` wire shape 一致。

## 推荐执行顺序摘要

1. 先补 RPC running/abort/steer/follow-up characterization tests。
2. 重构 RPC 为 async select loop。
3. 增加 stable `api` facade 和 public API docs。
4. 抽 shared request builder，消除 CLI/interactive 装配重复。
5. 分批拆 `interactive/app.rs`。
6. 引入 allow-all capability、typed tool args 和工具 common/walk helper。
7. 清理 `SessionPromptOptions` / `PrintModeOptions` 语义重复。

这条路线优先修正实际行为缺陷，同时给其他 crate 一个稳定依赖面。后续再做内部拆分和工具层收敛，能最大限度降低迁移期重构对现有功能的冲击。
