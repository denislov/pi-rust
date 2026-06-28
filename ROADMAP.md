# pi → Rust 移植路线图（索引）

> 状态：M0–M11 完成 · 最后核对：2026-06-28
> 适用范围：`pi-rust/` 工作区（把 TypeScript `pi` monorepo 移植为 Rust）
> 参考基准：`pi/`（上游 TS 实现，行为权威来源）

本文件是**导航索引**。详细内容已按里程碑拆分到 [`docs/roadmap/`](docs/roadmap/)，防止单文件膨胀。

---

## 0. 目标与约束（本轮确认）

**终极目标**：完全对标 `pi` 的全部功能。实现时**基于 Rust 特性重构，不照搬 TS**。

| 约束 | 决策 |
|---|---|
| 对标范围 | **核心优先，周边后置**（导出/分享/包管理等排到 [M13](docs/roadmap/M13-peripherals.md)） |
| 配置 / 认证 | Rust 原生格式，**不**读 pi 的 settings/auth |
| 会话 | session JSONL 与 pi **互通**（唯一线缆兼容点，需保持不漂移） |
| 插件系统 | **不继承** pi 的 TS 模块系统；自研**分层：Rust trait 内核 + Lua 脚本层**（[M12](docs/roadmap/M12-plugin-system.md)） |

---

## 1. 完成度（已核实）

| 包 / crate | TS 源码 | Rust 源码 | 粗略覆盖 | 状态 |
|---|---:|---:|---:|---|
| `pi-ai` | 30,553 | 30,694¹ | ~90% | ✅ M2+M8，缺 Vertex provider（见下方风险） |
| `pi-agent-core` | 8,067 | 10,187 | ~126%² | ✅ M4+M9 |
| `pi-coding-agent` | 48,013 | 24,019 | ~50% | ✅ M5/M7/M10，生态/扩展待补（M12/M13） |
| `pi-tui` | 11,696 | 10,616 | ~91% | ✅ M6+M11 基础设施，交互 polish 持续 |
| `pi-mom`/`pi-pods`/`pi-web-ui` | — | 各 14 | — | ⬜ 空壳，范围未定（[cross-cutting](docs/roadmap/cross-cutting.md)） |
| **合计** | **~98K** | **~75.5K** | **~77%** | — |

> ¹ `pi-ai` 中 9,591 行 `.rs` + 21,103 行 `models_generated.json`。
> ² `pi-agent-core` Rust 行数多于 TS 是因为额外实现了 harness、compaction、
>   skills/prompt templates 加载等 TS 中分布在多个包的逻辑。
> "TS 源码"为 `src/**/*.ts`（去 `*.test.ts`）行数，来自上游 `pi/` 仓库。
> 健康度：截至 2026-06-28 核对工作区编译通过，`cargo fmt --check`、
> `cargo check --workspace`、`cargo test --workspace`、`scripts/tui-smoke.sh` 全绿。
> 完成信号以实际代码与 Git 提交为准。

**已完成项** → [docs/roadmap/done.md](docs/roadmap/done.md)

---

## 2. 依赖关系

```
pi-ai  ──┬──>  pi-agent-core  ──>  pi-coding-agent
         │                              ▲
pi-tui ──┴──────────────────────────────┘   (交互模式时接入)
```

---

## 3. 里程碑（剩余工作，核心优先排序）

| # | 里程碑 | 状态 | 依赖 | 详情 |
|---|---|---|---|---|
| **M7** | 配置 + 认证基座（Rust 原生） | ✅ | — | [M7-config-auth](docs/roadmap/M7-config-auth.md) |
| **M8** | pi-ai provider 广度 + 认证 | ✅ | M7 | [M8-provider-breadth](docs/roadmap/M8-provider-breadth.md) |
| **M9** | agent-core harness 完备 | ✅ | — | [M9-agent-harness](docs/roadmap/M9-agent-harness.md) |
| **M10** | 资源发现 + 输入路径 | ✅ | M7 | [M10-resources-input](docs/roadmap/M10-resources-input.md) |
| **M11** | 交互体验补全（含 TUI-7 发布门） | ✅ | M7,M9,M10 | [M11-interactive-ux](docs/roadmap/M11-interactive-ux.md) |
| **M12** | 插件系统（Rust trait + Lua） | ⬜ | M9,M11 | [M12-plugin-system](docs/roadmap/M12-plugin-system.md) |
| **M13** | 周边能力（后置） | ⬜ | M11 | [M13-peripherals](docs/roadmap/M13-peripherals.md) |
| 横切 | 辅助 crate / 兼容 / 风险 | — | — | [cross-cutting](docs/roadmap/cross-cutting.md) |

> 旧标签映射：旧"M7 周边能力"拆进 M7–M13；TUI-7 smoke 并入 M11 作交互发布门；TUI-8 剩余并入 M11。

---

## 4. 当前焦点（M12 → M13）

已完成 M0–M11。剩余的未落地工作：

1. **M12 插件系统** — 最大缺失子系统（对标 TS extension 体系）。分两期：Rust trait 内核 + mlua Lua 脚本层。
2. **M13 周边能力** — 完整 HTML 导出 parity、gist 分享、package manager（`/copy`、`/export`、`/import`、`/clone` 基础能力已落地）。
