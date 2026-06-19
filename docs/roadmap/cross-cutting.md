# 横切项：辅助 crate · 兼容约束 · 风险

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md)
> 不属于单一里程碑、但贯穿全程的事项。

## 1. 辅助 crate 定位（pi-mom / pi-pods / pi-web-ui）
- 三者均为 14 行 `cargo new` 桩，**不在** `AGENTS.md` 的四包迁移图内，无对应 TS 来源，无设计文档。
- **行动项**：投入前先明确各自目标范围（对应 TS 侧什么，或上游新需求）；否则保持空壳。
- 决策选项：定义范围并排里程碑 / 暂缓 / 移除。**当前默认：暂缓**，待四包对标推进到一定程度再评估。

## 2. 兼容约束（贯穿 M3 / M7 / M10）
- **会话**：session JSONL 与 pi **互通**（可共用会话目录）。这是当前唯一要求的线缆兼容点；
  在推进 M9/M10 时需**核验并保持**该格式不漂移（建议加一个"读 pi 产出的 JSONL"的 fixture 测试）。
- **配置 / 认证**：用 Rust 原生格式，**不**要求读 pi 的 `settings.json`/`auth.json`（见 [M7](M7-config-auth.md)）。
- **事件协议 / wire JSON**：provider 层与 pi 保持字节级一致（serde 桥接），由各 provider 测试守护。

## 3. 关键约束（沿用各 spec）
- **离线优先**：所有测试不依赖真实 provider key；用 faux provider / fixture / 单元测试证明正确性。
- **惯用 Rust**：snake_case + enum + `Result`/typed error；不照搬 TS 结构，按 Rust 特性重构。
- **小 crate 对应 TS 包边界**，不向根 package 堆叠跨切面代码。
- **TUI 不用 ratatui**，坚持 pi 的字符串组件 + 差分输出模型。
- **工作树纪律**：不回退/覆盖他人改动；`pi/` 与 `pi-rust/` 是两个独立 git 仓库，分别操作。

## 4. 风险
| 风险 | 说明 | 缓解 |
|---|---|---|
| 模型表漂移 | 生成式注册表已落地，但 TS 上游模型更新需重新生成 | 保留生成脚本，定期重生成 |
| 会话格式漂移 | M9/M10 改动可能破坏与 pi 的 JSONL 互通 | fixture 测试守护（读 pi 产出） |
| 主题系统体量大 | 256 色/能力探测/跨终端一致性（[M11](M11-interactive-ux.md)） | 拆细粒度迭代 |
| 跨终端差异 | 宽度/按键协议不一致 | TUI-7 smoke 套件覆盖主流终端 |
| 插件沙箱安全 | Lua 脚本越权访问（[M12](M12-plugin-system.md)） | capability 白名单 + `ExecutionEnv` 受控访问 |
| 全局可变注册表 | provider 注册为进程级全局 | 测试用唯一 api id 隔离 |
| 认证子系统体量 | OAuth/SigV4/WebSocket 各自独立（[M8](M8-provider-breadth.md)） | 逐 provider spec→plan→实现 |
| 辅助 crate 无方向 | 范围未定，贸然投入有返工风险 | 先定范围再投入 |
