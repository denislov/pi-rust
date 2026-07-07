# Footer 实时 usage 更新 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Completed status, 2026-07-07: interactive footer realtime usage is implemented and recorded in `docs/TODO.md`. `AssistantMessageCompleted` carries usage, `CodingEventBridge` emits per-message `UsageUpdate`, and the old turn-level usage update path has been removed.

**Goal:** 让 interactive TUI footer 的 token/cost/上下文在每个 assistant message 完成时即时刷新，对标 TypeScript `pi` 的 `message_end` + pull 重算语义，而不是等到整个 prompt turn 完成。

**Architecture:** Rust 当前是 push/缓存模型——`root.stats` 只在 `finish_coding_prompt`（turn 完成）时由 `apply_success_usage` 写入一次，且 `CodingAgentEvent::AssistantMessageCompleted` 不携带 usage。改为让事件流携带 per-message usage，由 `CodingEventBridge` 在每个 `AssistantMessageCompleted` 累加并发 `UiEvent::UsageUpdate`，`root.apply_events` 收到后立即更新 `FooterStats`，下次 `footer()` 渲染即刷新。同时移除 `apply_success_usage` / `update_usage` turn 级机制（避免重复累加，并取消"内部辅助任务 usage 不计入"的区分，对齐 TS 遍历所有 entries 的语义）。

**Tech Stack:** Rust edition 2024, `pi-coding-agent` crate, `pi-ai::types::{Usage, AssistantMessage}`.

## Global Constraints

- 不要求真实 model provider key；所有测试用 faux provider / 构造的 `Usage`。
- `Usage` 已 `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]`（`pi-ai/src/types/usage.rs`）。
- RPC wire 序列化的是 `ProtocolEvent`（经 `CodingProtocolEventAdapter` 转换），不是 `CodingAgentEvent`——给 `AssistantMessageCompleted` 加字段**不**影响 RPC wire 协议。
- 不保留死代码：`apply_success_usage` 和 `update_usage` 一旦被事件层取代即移除（遵循 `AGENTS.md`）。
- 改动后运行 `cargo fmt --check`、`cargo test --workspace`、`cargo check --workspace`。

## 行为变化（需知晓）

- **Before:** self-healing edit / branch summary / branch navigation 等 `update_usage=false` 的内部辅助任务的 token/cost 不计入 footer。
- **After:** 这些任务的每个 assistant message 完成时也会实时累加进 footer，对齐 TS `FooterComponent.render` 遍历所有 session entries 的语义。这些本就是用户消耗的真实 token，计入是合理的。

---

## File Structure

| 文件 | 职责 | 改动 |
|------|------|------|
| `crates/pi-coding-agent/src/coding_session/event.rs` | `CodingAgentEvent` enum | `AssistantMessageCompleted` 加 `usage: Usage` 字段 |
| `crates/pi-coding-agent/src/coding_session/event_service.rs` | `map_agent_event` | `AgentDone` 分支 emit 时带 `message.usage.clone()` |
| `crates/pi-coding-agent/src/interactive/event_bridge.rs` | `CodingEventBridge::handle` | `AssistantMessageCompleted` 累加 `total_*` + 发 `UsageUpdate`；新增 `calculate_context_tokens` helper |
| `crates/pi-coding-agent/src/interactive/loop.rs` | turn 完成收尾 | 移除 `apply_success_usage`、`finish_coding_prompt` 的 `update_usage` 参数及调用 |
| `crates/pi-coding-agent/src/interactive/prompt_task.rs` | `CodingPromptTaskResult` | 移除 `update_usage` 字段及 4 处构造 |
| `crates/pi-coding-agent/tests/interactive_event_bridge.rs` | bridge 测试 | 更新 `AssistantMessageCompleted` 构造 + 断言 `[AssistantDone, UsageUpdate]` |
| `crates/pi-coding-agent/tests/protocol_events.rs` | protocol 测试 | 2 处构造加 `usage` |
| `crates/pi-coding-agent/src/protocol/rpc/events.rs` | RPC adapter 测试 | 构造加 `usage` |
| `crates/pi-coding-agent/src/coding_session/event_service.rs` (tests) | map_agent_event 测试 | 构造加 `usage` |
| `crates/pi-coding-agent/src/interactive/app.rs` | footer 测试 | 视情况调整 |
| `crates/pi-coding-agent/docs/TODO.md` | 项目 checklist | 标注进度 |

---

### Task 1: `AssistantMessageCompleted` 携带 `usage`

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs:210-216`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs:678-684`
- Modify (test fixtures): `crates/pi-coding-agent/src/coding_session/event_service.rs:1026`, `crates/pi-coding-agent/tests/interactive_event_bridge.rs:73`, `crates/pi-coding-agent/tests/protocol_events.rs:71,150`, `crates/pi-coding-agent/src/protocol/rpc/events.rs:53`

**Interfaces:**
- Produces: `CodingAgentEvent::AssistantMessageCompleted { ..., usage: Usage }` — 后续 task 依赖此字段。

- [x] **Step 1: 加字段**

`event.rs` `AssistantMessageCompleted` 变体：
```rust
AssistantMessageCompleted {
    operation_id: String,
    turn_id: String,
    message_id: Option<String>,
    final_text: String,
    usage: Usage,
},
```
确认 `event.rs` 顶部已 `use pi_ai::types::Usage;`（若无则加）。

- [x] **Step 2: emit 点带 usage**

`event_service.rs:678`：
```rust
AgentEvent::AgentDone { message } => {
    vec![CodingAgentEvent::AssistantMessageCompleted {
        operation_id: context.operation_id.clone(),
        turn_id: context.turn_id.clone(),
        message_id: context.assistant_message_id.clone(),
        final_text: assistant_text(&message.content),
        usage: message.usage.clone(),
    }]
}
```

- [x] **Step 3: 更新所有测试构造点**

每个构造 `AssistantMessageCompleted { .. }` 的地方加 `usage: Usage::default()`（或测试专用值）。位置（共 5 处）：`event_service.rs:1026`、`tests/interactive_event_bridge.rs:73`、`tests/protocol_events.rs:71`、`tests/protocol_events.rs:150`、`protocol/rpc/events.rs:53`。确保各文件 `use pi_ai::types::Usage;`（或 `pi_ai::types::Usage`）。

- [x] **Step 4: 编译验证**

Run: `cargo check -p pi-coding-agent`
Expected: PASS（无编译错误；消费端用 `{ .. }` 或 `{ final_text, .. }` 的 match 不受影响）

- [x] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(coding-agent): carry usage in AssistantMessageCompleted event"
```

---

### Task 2: bridge 实时累加并发 `UsageUpdate`（TDD）

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs:82` 及 `CodingEventBridge` impl
- Test: `crates/pi-coding-agent/tests/interactive_event_bridge.rs`

**Interfaces:**
- Consumes: `AssistantMessageCompleted { usage, .. }` (Task 1)
- Produces: `UiEvent::UsageUpdate { input, output, cache_read, cache_write, cost, context_tokens: Some(_) }` 紧随 `UiEvent::AssistantDone`

- [x] **Step 1: 写失败测试**

在 `tests/interactive_event_bridge.rs` 修改 `coding_event_bridge_maps_assistant_events`，断言 `AssistantMessageCompleted` 产出 `[AssistantDone, UsageUpdate]`：
```rust
let done = bridge.handle(&CodingAgentEvent::AssistantMessageCompleted {
    operation_id: "op_1".to_string(),
    turn_id: "turn_1".to_string(),
    message_id: Some("msg_1".to_string()),
    final_text: "hello".to_string(),
    usage: Usage {
        input: 100,
        output: 50,
        cache_read: 0,
        cache_write: 0,
        total_tokens: 150,
        cost: Cost { input: 0.1, output: 0.2, cache_read: 0.0, cache_write: 0.0 },
    },
});
assert_eq!(
    done,
    vec![
        UiEvent::AssistantDone,
        UiEvent::UsageUpdate {
            input: 100,
            output: 50,
            cache_read: 0,
            cache_write: 0,
            cost: 0.3,
            context_tokens: Some(150),
        },
    ]
);
```
加一个累积测试：两次 `AssistantMessageCompleted` 后 `UsageUpdate.input` 累加。

- [x] **Step 2: 跑测试验证失败**

Run: `cargo test -p pi-coding-agent --test interactive_event_bridge coding_event_bridge_maps_assistant_events`
Expected: FAIL（当前只产 `[AssistantDone]`）

- [x] **Step 3: 实现**

`event_bridge.rs`：加 `calculate_context_tokens` helper（从 `loop.rs` 移植逻辑）；`AssistantMessageCompleted` 分支：
```rust
CodingAgentEvent::AssistantMessageCompleted { usage, .. } => {
    self.total_input = self.total_input.saturating_add(usage.input);
    self.total_output = self.total_output.saturating_add(usage.output);
    self.total_cache_read = self.total_cache_read.saturating_add(usage.cache_read);
    self.total_cache_write = self.total_cache_write.saturating_add(usage.cache_write);
    self.total_cost += usage.cost.input + usage.cost.output
        + usage.cost.cache_read + usage.cost.cache_write;
    let context_tokens = calculate_context_tokens(usage);
    vec![
        UiEvent::AssistantDone,
        UiEvent::UsageUpdate {
            input: self.total_input,
            output: self.total_output,
            cache_read: self.total_cache_read,
            cache_write: self.total_cache_write,
            cost: self.total_cost,
            context_tokens: Some(context_tokens),
        },
    ]
}
```
注意：`handle(&self, ...)` 签名需改为 `&mut self`（累加器可变）。检查所有 `bridge.handle(...)` 调用点（`loop.rs:1927`、`loop.rs:1341`、`tests`）已用 `&mut bridge`。

- [x] **Step 4: 跑测试验证通过**

Run: `cargo test -p pi-coding-agent --test interactive_event_bridge`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(coding-agent): bridge emits UsageUpdate on each AssistantMessageCompleted"
```

---

### Task 3: 移除 `apply_success_usage` / `update_usage` 机制

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/loop.rs`（移除 `apply_success_usage`、`calculate_context_tokens` 若已移至 bridge、`finish_coding_prompt` 的 `update_usage` 参数、1977/2103-2104 调用）
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs:35`（移除字段）及 668/887/1199/1261 构造点

**Interfaces:**
- 消费 Task 2 的实时 `UsageUpdate`；不再有 turn 级 usage 写入。

- [x] **Step 1: 移除 loop.rs 累加逻辑**

删除 `fn apply_success_usage`。`finish_coding_prompt` 签名去掉 `update_usage: bool` 参数，删除 `if update_usage { apply_success_usage(...) }` 块。删除 loop.rs:1977 `apply_success_usage(root, &result.outcome.final_message.usage);`（AgentInvocation 路径）。更新 `finish_coding_prompt` 的两个调用点（1955、1966）去掉 `result.update_usage` 实参。若 `calculate_context_tokens` 仅被 `apply_success_usage` 使用则一并删除（Task 2 已在 bridge 复制）。

- [x] **Step 2: 移除 prompt_task.rs `update_usage`**

删除 `CodingPromptTaskResult.update_usage` 字段；删除 4 处构造（668/887/1199/1261）的 `update_usage: ...,` 行。

- [x] **Step 3: 编译 + 测试**

Run: `cargo check -p pi-coding-agent && cargo test -p pi-coding-agent`
Expected: PASS。若 `app.rs` footer 测试因 `UsageUpdate` 时机变化失败，按新语义修正（footer 在 `apply_events([UsageUpdate])` 后反映新值）。

- [x] **Step 4: Commit**

```bash
git add -A && git commit -m "refactor(coding-agent): remove turn-level apply_success_usage in favor of event-driven usage"
```

---

### Task 4: 端到端验证 + TODO 更新

**Files:**
- Verify: `crates/pi-coding-agent/src/interactive/app.rs` footer 测试
- Modify: `docs/TODO.md`

- [x] **Step 1: 补 footer 实时更新测试**

在 `app.rs` 测试模块加：构造 `root`，`apply_events([AssistantDelta, AssistantDelta, UsageUpdate{...}])`，断言 `root.footer(80)` 含新 token/cost/context。再 `apply_events([UsageUpdate{...更大}])`，断言 footer 反映累加值。

- [x] **Step 2: 全量验证**

Run: `cargo fmt --check && cargo test --workspace && cargo check --workspace`
Expected: 全 PASS

- [x] **Step 3: 更新 TODO.md**

在 `docs/TODO.md` 相关 phase 加进度注记（footer 实时 usage 已对齐 TS `message_end` 语义）。

- [x] **Step 4: Commit**

```bash
git add -A && git commit -m "test+docs: verify footer realtime usage update and update TODO"
```

---

## Self-Review

1. **Spec coverage:** "token/cost/上下文实时更新" → Task 2 每个 assistant message 完成发 `UsageUpdate` 覆盖。✓
2. **重复累加:** Task 3 移除 `apply_success_usage`，bridge 成唯一累加源。✓
3. **compaction:** `CompactionCompleted` 仍发 `UsageUpdate { total_*(真实累计), context_tokens: None }`——`total_*` 现是真实累计（对齐 TS 保留累计，只清 context 估算）。✓ 现有 compaction 测试（独立 bridge，无前置 assistant）仍断言 0，保持 PASS。✓
4. **`handle` 签名 `&mut self`:** loop.rs:1927/1341 已 `&mut coding_bridge`/`&mut bridge`。✓
5. **RPC wire:** 加字段不影响 `ProtocolEvent` 序列化。✓
6. **类型一致:** `calculate_context_tokens` 在 bridge 和（移除前的）loop 同语义；Task 3 删除 loop 副本。✓
