# pi-rust Roadmap Index

> 状态：Flow-centered runtime Phase 1-4 已落地；下一阶段为 Phase 5 Plugin Kernel
> 最后核对：2026-07-01
> 适用范围：`pi-rust/` 工作区

本文件是当前路线图入口。详细执行清单以 [`docs/TODO.md`](docs/TODO.md) 为准；旧 M7-M13 文档保留为 TS parity 背景和历史拆分索引。

---

## 1. 当前方向

`pi-rust` 不再按“逐字迁移 TypeScript pi”推进。当前目标是以 TS `pi` 作为行为/产品参考，以 PocketFlow 的显式图编排作为架构参考，构建 Rust-native、可声明、可观测、可测试的 agent runtime。

| 领域 | 当前决策 |
|---|---|
| 产品目标 | 覆盖 TS pi 的核心能力，但不复制 TS 内部结构 |
| 会话持久化 | 使用 Rust-native `session.json` + `events.jsonl` typed event log，不保持 TS session JSONL 互通 |
| Runtime 架构 | `CodingAgentSession` 作为产品所有者；`PromptTurnFlow` 负责产品 turn；`AgentTurnFlow` 负责低层 agent loop |
| Adapter | print/json/RPC/interactive 消费统一 `CodingAgentEvent` 或其边界 adapter |
| 插件系统 | 先做 Rust trait 内核边界，再考虑 Lua 脚本层 |
| 测试策略 | 默认 deterministic/offline；真实 provider 或终端 smoke 必须显式 opt-in |

---

## 2. 当前完成状态

| Phase | 状态 | 说明 |
|---|---|---|
| Phase 1 | Done | `CodingAgentSession` skeleton、Rust-native session log、transaction/replay 基础已落地 |
| Phase 2 | 主要完成 | `PromptTurnFlow`、runtime snapshot、session finalization、print/json convergence 已落地；测试/check 子项仍在 TODO 中跟踪 |
| Phase 3 | 主要完成 | RPC/interactive adapter convergence、Rust-native resume/tree/fork/clone/compact、旧 JSONL 产品路径清理已落地；测试/check 子项仍在 TODO 中跟踪 |
| Phase 4 | Done | `AgentTurnFlow` 已成为 `Agent::run()` 的低层 runtime entrypoint；旧 `agent_loop` 仅保留兼容 wrapper |
| Phase 5 | Open | Plugin Kernel：registry、capability-scoped providers、tool/hook integration、failure isolation |
| Phase 6 | Open | Advanced Flow workflows：manual compaction/export/branch/plugin-load/subagent 等 first-party flows |

---

## 3. 主要文档入口

- 当前执行清单：[`docs/TODO.md`](docs/TODO.md)
- 架构总览：[`docs/superpowers/ARCHITECTURE.md`](docs/superpowers/ARCHITECTURE.md)
- Flow-centered 设计：[`docs/superpowers/specs/2026-06-29-flow-centered-runtime-architecture-design.md`](docs/superpowers/specs/2026-06-29-flow-centered-runtime-architecture-design.md)
- Flow-centered 实施计划：[`docs/superpowers/plans/2026-06-29-flow-centered-runtime-architecture-plan.md`](docs/superpowers/plans/2026-06-29-flow-centered-runtime-architecture-plan.md)
- Rust-native session format：[`docs/superpowers/specs/2026-07-01-rust-native-session-format.md`](docs/superpowers/specs/2026-07-01-rust-native-session-format.md)
- 横切约束与风险：[`docs/roadmap/cross-cutting.md`](docs/roadmap/cross-cutting.md)

---

## 4. 旧 roadmap 文档

这些文档仍可用于理解早期 TS parity 缺口，但不再作为当前架构约束来源：

- [`docs/roadmap/M7-config-auth.md`](docs/roadmap/M7-config-auth.md)
- [`docs/roadmap/M8-provider-breadth.md`](docs/roadmap/M8-provider-breadth.md)
- [`docs/roadmap/M9-agent-harness.md`](docs/roadmap/M9-agent-harness.md)
- [`docs/roadmap/M10-resources-input.md`](docs/roadmap/M10-resources-input.md)
- [`docs/roadmap/M11-interactive-ux.md`](docs/roadmap/M11-interactive-ux.md)
- [`docs/roadmap/M12-plugin-system.md`](docs/roadmap/M12-plugin-system.md)
- [`docs/roadmap/M13-peripherals.md`](docs/roadmap/M13-peripherals.md)

当旧里程碑文档与 `docs/TODO.md`、Flow-centered specs 或当前代码冲突时，以后者为准。
