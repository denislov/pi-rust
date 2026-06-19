# M12 — 插件系统（自研：Rust trait 内核 + Lua 脚本层）

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M9](M9-agent-harness.md)（抽象/事件）、[M11](M11-interactive-ux.md)（UI 原语）
> 决策：**不继承 pi 的 TS 动态模块插件系统**，自研分层方案。

## 目标
为 pi-rust 设计一套**分层**扩展系统：Rust trait 定义扩展点（host 接口），mlua 把这些扩展点
暴露给用户 Lua 脚本。第一方扩展直接实现 trait，用户扩展写 Lua（无需 Rust 工具链）。

## 架构

```
         ┌─────────────────────────────────────┐
         │  扩展点 trait（host 接口，Rust）       │
         │  - ToolProvider   自定义工具          │
         │  - CommandProvider 自定义 slash 命令   │
         │  - HookProvider   生命周期钩子         │
         │  - UiProvider     对话框/overlay/补全  │
         │  - KeybindProvider 键位注册            │
         └───────────────┬─────────────────────┘
                         │ 实现
           ┌─────────────┴─────────────┐
           │                           │
   第一方/编译期扩展            mlua 桥接层（A 之上）
   （直接 impl trait）          把 trait 暴露给 .lua 脚本
                                       │
                                  用户 .lua 插件（沙箱、热重载）
```

## 分两期推进

### Phase A — 原生 Rust 扩展 trait 内核
- 定义扩展点 trait（上图 5 类）与注册表（registry）。
- 扩展生命周期：发现 → 加载 → 注册能力 → 卸载/reload（`/reload` 命令在 [M11](M11-interactive-ux.md)）。
- 第一方扩展直接 impl trait 并静态注册，跑通端到端（如一个示例自定义工具 + 自定义 slash 命令）。
- 依赖 [M9](M9-agent-harness.md) 的事件/钩子系统与 FileSystem/Shell 抽象（沙箱基础）。

### Phase B — mlua Lua 脚本层
- 引入 `mlua`（绑定 Lua 5.4 / LuaJIT，按需选）。
- 把 Phase A 的 trait 能力面通过 Lua API 暴露：脚本可注册工具/命令/钩子/键位、调用受限的 host 能力。
- **沙箱**：限制 Lua 可见的 host 能力（经 `ExecutionEnv` 受控访问文件/shell），默认最小权限。
- 插件发现与加载：从 `~/.pi/` 与项目目录加载 `.lua`，支持热重载。
- 稳定的 Lua API 版本约定（避免破坏用户脚本）。

## 关键设计决策（待 spec 细化）
- Lua 运行时：`mlua` 的 lua54 vs luajit（性能 vs 兼容）。
- 沙箱粒度：能力白名单（capability-based）。
- 错误隔离：单个插件崩溃不影响主循环（panic catch / Lua error 边界）。
- 与 [M11](M11-interactive-ux.md) UI 原语的接口（Lua 能否绘制组件、绘制到何种程度）。

## 验收 / 测试（离线优先）
- Phase A：示例第一方扩展注册的工具/命令在 faux provider 下被调用。
- Phase B：fixture `.lua` 脚本注册能力并触发钩子；越权访问被沙箱拒绝的断言；插件 panic 不传播的断言。

## 不做
- pi 的 npm/git 包安装式插件分发（→ 若需要，归 [M13](M13-peripherals.md) 的包管理讨论）。
