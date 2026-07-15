# `pi-rust` Breaking Architecture Convergence 执行计划

> 目标版本：Rust crates `0.2.0`，live protocol family major `2`
>
> 基线提交：`605bb8a chore: checkpoint crates before breaking convergence`
>
> 策略：激进清理（允许 Rust API、live protocol 和 product event surface breaking）
>
> 状态：执行中（M0/M1 已完成，M2--M6 进行中，M7 未开始）
>
> 依据：`docs/architecture.md`、`docs/code-cleanup-strategies.md` 和 2026-07-15 工作区源码

## 执行快照（2026-07-15）

| Milestone | 状态 | 已落地内容 | 下一退出条件 |
|---|---|---|---|
| M0 | 完成 | contract inventory、session compatibility fixtures、dead-code inventory、architecture gates | 持续保持 gates 通过 |
| M1 | 完成 | WP1.1/WP1.2 已完成；scoped provider runtime、单一 agent loop、lower-level facade 收窄、`pi-coding-agent` root deprecated re-export 删除 | 保持 facade boundary 不回退 |
| M2 | 进行中 | WP2.1 admission descriptor 已统一 class/dispatch/kind 合同（`c3df909`），admission record 已保存 operation identity、capability generation 和 root/child lineage（`e4c8b2a`）；WP2.2 scheduler core 已接入 canonical dispatchers，提供 typed rejection（`68cc0dc`、`788238d`）；prompt/compact/async vertical slice 已迁移（`4e8515b`）；invocation/delegation 与 profile/runtime slice 已由 canonical operation 集成测试覆盖，delegated child operation 已进入 scheduler lineage admission（`60b61c1`） | 完成 workflow/control 与 session-navigation slices，扩展 child permit 到 cancellation/terminal association |
| M3 | 进行中 | WP3.1 event/manifest decoder 已改为 explicit schema/version dispatch；v2 event 缺失 `session_sequence` 仍可兼容 replay，unknown decoder fail closed 并提供恢复建议（`88ddab4`）；WP3.2 已强制 durable sequence 从 1 连续递增，legacy 无 sequence 行按逻辑行号归一化（`b6e1218`） | 定义 append CAS/idempotency 与 partial commit 合同，扩展故障注入矩阵 |
| M4 | 进行中 | WP4.3 已完成 `pi-coding-agent/src` file-level dead-code suppression 清零；无 production consumer 的 capability/Flow/client/prompt 表面已删除，仅测试使用的构造、观察、取消验证、durable fixture 与 execution-env adapters 精确限制为 `cfg(test)`。旧 client/submission 投影由 SnapshotCoordinator/public connection 取代（`3208364`）；session-log durable 生产路径保留（`ec447f7`）；prompt 平行 observation 已删除（`141b184`）；architecture Non-Goal 中的 arbitrary plugin Flow extension 空壳已整组删除并由 boundary guard 禁止回归（`b0dcf46`）。WP4.2 删除未实现的 revocation mode（`0730e44`）。 | 审计其余 plugin/raw service capability escape，完成 generation/revocation 与 snapshot audit 统一 |
| M5 | 进行中 | WP5.1 已删除 `CodingAgentProductEvent` 顶层 deprecated `family`/`kind` 字符串字段，public wire 仅保留 typed event family/payload kind；测试消费者改为 typed family + kind 查询（`1f09f1f`）。WP5.2 public snapshot cursor 已增加稳定 `stream_id`（`6f3df8c`）、`snapshot_protocol_major`（`134e374`）并统一使用 camelCase wire shape；fresh snapshot 与 reconnect replay 统一填充 session identity/version，RPC prompt 已删除裸 `afterSnapshotSequence` 并要求完整 cursor（`b03a0bc`）；canonical `reconnect_from_cursor` 已校验 stream/major 并复用 atomic recovery boundary（`ea11a09`）。WP5.3 RPC queue 已拆分 data/control lanes，overflow recovery 优先于已满的 data lane（`8faa190`） | 继续定义 terminal/control 优先级、shutdown drain 和 reconnect overlap，并完成剩余 adapter 收敛 |
| M6-M7 | M6 进行中；M7 未开始 | M6/WP6.2 的 session facade/module split 已覆盖 query、dispatcher、connection、lifecycle、submission、control、admission/capability、prompt 与其余 operation helpers；construction options、profile/plugin/runtime resolution、cwd 与 replay-derived owner state 也已归入 `session_lifecycle.rs`（`92a0920`）。`coding_session/mod.rs` 仅保留 module graph、facade exports/import aliases 与 session state layout；legacy internal client facade、submission projection 与无消费者 ClientService pass-through 已删除（`3208364`）；file-level dead-code suppression 已为零（`141b184`）。WP6.3 已将 session tests 与 fixture bridges 分离到独立 test-only owners。 | 执行 hidden fallback、item-level suppression 与 facade 完成审计；随后生成 API diff/migration guide 并进入 release train |

完成审计基线：`pi-coding-agent/src` 的 file-level suppression 为 `0`；plugin Flow extension 与 registry/service 高价值死表面删除后，item-level `allow(dead_code)` 从 102 降至 72。根据项目推进决策，不要求本阶段逐项清零剩余 item-level suppression；只处理会掩盖架构分叉、raw service escape、旧 facade 或 production 热路径冗余的条目。M4/M6 后续转入 hidden fallback、capability escape、facade/API diff 审计。

已提交检查点：

- `e561134`：建立 architecture convergence baseline。
- `0544b97`：删除 retired agent loop。
- `8e36a8a`：`pi-agent-core` 强制 scoped provider streaming。
- `465f236`：修正 product Flow 测试的显式 provider 注入。
- `7354cae`：证明两个同名 API 的并行 scoped registry 无串扰。
- `377d1f5`：由 `CodingAgentSession` 持有并向 operation runtime 注入 scoped `AiClient`。
- `6554f63`：将 scoped `AiClient` 贯穿 CLI、print/JSON、RPC 和 interactive adapters。
- `80d2b86`：删除 product global provider test bridge，并将全部 product provider fixtures 迁到 scoped client。
- `605bb8a`：暂存并提交 `crates/` 全部代码变更，作为激进清理和 breaking release 的回滚检查点。
- `2842ce8`：迁移 `pi-ai` provider 测试到 `pi_ai::api`，将 `registry` 模块收窄为 crate-private 实现。
- `7dcb1f0`：删除 4 个仅验证 serde derive 命名/标签的低价值单元测试，保留集成 round-trip 与 wire-shape 覆盖。
- `7ff5707`：删除 interactive `UiProjection::last_sequence()` 未使用 accessor，测试直接验证 projection cursor。
- `b80b6f0`：将 Flow 和 transcript 合同集中到 `pi_agent_core::api`，迁移 product/test consumers，并把两个模块改为 crate-private。
- `a7277db`：迁移 resource consumers 到 `pi_agent_core::api`，将 `resources` 模块改为 crate-private。
- `e1db230`：迁移 `DiagnosticSeverity` consumers 到 `pi_agent_core::api`，将 `types` 模块改为 crate-private。
- `6550ed9`：将 branch summary、proxy、shell output、truncate、session context 合同集中到 `pi_agent_core::api`，迁移测试消费者并隐藏实现模块。
- `601b486`：将 compaction estimate/prepare/summarize 合同集中到 `pi_agent_core::api`，迁移 product/test consumers 并隐藏 `compaction` 模块。
- `a966da3`：将 AgentTurnContext、AgentTurnFlow 及节点合同集中到 `pi_agent_core::api`，迁移行为测试并隐藏 `agent_turn_flow` 模块；具体节点暂因测试隔离需求保留。
- `e629a8d`：迁移仓内所有 root deprecated API consumers 到 `pi_coding_agent::api` 或 owner module，删除 `CliArgs`、`CliError`、`CliRunOptions`、`run_cli`、`run_print_mode`、`builtin_tools` 等 root-level compatibility re-export，并将 API boundary guard 改为验证删除结果。
- `c3df909`：将 `CodingAgentOperation` descriptor 与内部 `Operation::metadata()` 的 admission class、dispatch mode、submitted kind 统一校验，建立 M2 scheduler 前的单一 operation contract 基线。
- `68cc0dc`：新增 `OperationScheduler` 和 `AdmissionRejection`，让 operation admission 统一经过 scheduler；read-only busy bypass、typed busy rejection、dispatch mismatch fail-closed、query classification 均有单元测试。
- `788238d`：将 `run_sync_operation`、`run_sync_mut_operation`、`run_operation` 三个 canonical dispatcher 直接迁移到 `OperationScheduler::admit`，删除 `IntentRouter::admit_operation`/`begin` 第二 admission 入口，并更新契约测试。
- `4e8515b`：删除剩余 `IntentRouter` operation admission 测试入口，统一 scheduler 的 dispatch mismatch 错误上下文，并以完整 coding-session 测试验证 vertical slice。
- `f31ede0`：新增 product runtime boundary guard，递归剥离 test-only 源码后禁止 scheduler/operation-control 所有者之外的直接 `begin` admission 绕行，并锁定三条 canonical dispatcher 的 scheduler 调用数量。
- `4aed69a`：修正 scheduler dispatch mismatch 错误，明确报告 required/received dispatcher，保持 fail-closed rejection 的可诊断性。
- `e4c8b2a`：扩展 `OperationAdmission` identity contract，保存 operation id、capability generation、root/parent lineage，并接入可校验的 idempotency key 字段。
- `60b61c1`：新增 scheduler child admission，要求 delegated child snapshot 持有非空 parent lineage；将 frozen parent snapshot 注入 direct invocation、自动 delegation 与 pending approval execution，同时分离 identity lineage 与 capability inheritance，避免收紧直接 invocation 的 profile 权限。
- `88ddab4`：将 manifest/event 读取改为显式 schema/version decoder dispatch；保留 v1 manifest 与 v2 event（含无 `session_sequence`）兼容，unknown decoder fail closed 并输出 schema/version/event id/recovery context。
- `b6e1218`：在 durable event read 和 append preflight 中共享连续 sequence validator；旧日志无 sequence 时按行号兼容归一化，显式跳号或重复 event sequence fail closed。
- `e2f3265`：移除 `capability_snapshot.rs` 文件级 dead-code suppression，删除无生产消费者的 plugin actor、tool-name enumeration、filesystem/shell require helpers 及其仅验证死表面的测试。
- `0730e44`：删除无生产实现的 `CancelMatchingOperations` revocation mode 及其 public event/protocol 映射，避免暴露不会实际取消 active operation 的能力声明。
- `6f3df8c`：为 `CodingAgentSnapshotCursor` 增加 `stream_id`，由 session id 提供稳定 stream identity，覆盖 fresh snapshot 与 reconnect replay cursor。
- `134e374`：为 public snapshot cursor 增加 `snapshot_protocol_major`，fresh/replay 使用同一 `UI_SNAPSHOT_PROTOCOL_VERSION`，让独立 cursor 消费者可执行 major boundary 校验。
- `ea11a09`：新增 `CodingAgentClientConnection::reconnect_from_cursor`，拒绝错误 stream 或 snapshot major，并将合法 cursor 路由到既有 atomic replay/live recovery boundary。
- `1f09f1f`：删除 `CodingAgentProductEvent` 顶层 deprecated `family`/`kind` 字符串字段，迁移 invocation/team/delegation/profile 测试辅助函数到 typed family + kind 断言。
- `b03a0bc`：RPC prompt 删除裸 `afterSnapshotSequence`，改用完整 `afterSnapshotCursor`（stream identity、snapshot major、event sequence、capability generation），并统一调用 `reconnect_from_cursor`。
- `8faa190`：RPC ProductEvent queue 拆分 data/control lanes，overflow recovery 不再与普通 delta 竞争同一满队列；增加 data lane 满载时 control 优先交付测试，并更新 RPC boundary guard。
- `2f6ae07`：迁移 print、harness print 和 runtime 集成测试到 `pi_coding_agent::api` 稳定 facade，清除已删除 root compatibility exports 的测试消费者。
- `a6982fd`：删除仅被 `#[cfg(any())]` replay 测试使用、无生产消费者的 `ProductEventReplayHandle` 及其 RPC test seam，减少 runtime dead-code surface。
- `3785330`：同步 product runtime boundary method ledger，明确删除 replay seam 后的 stable facade 允许方法集合。
- `10f5e84`：确认 compact cancellation authority 只有单元测试消费者，将 `CompactCancellationHandle`、rejection 类型和 session accessor 限定为 `cfg(test)`，避免测试控制面进入 production facade。
- `b140574`：删除无生产消费者的 `ClientService::mark_terminal/acknowledge/detach` pass-through，移除 `CodingAgentSession::load_plugins` broad wrapper，测试改用 canonical `run_operation(Operation::PluginLoad)`；同时将 scheduler/test-only constructors 收窄到 `cfg(test)`。
- `5639a39`：删除不可达 `SubmittedOperationStatus::Accepted`、public `Accepted` status、`StaleClient`/`ReceiptCapacityExceeded`/`StaleClientConnection` errors、无调用 `validate_handle` 及未消费 snapshot acknowledged projection；保留真实 `Running -> Terminal` 与 shutdown recovery contract。
- `c92f77a`：将 `CodingAgentSession` capabilities/view/profile/plugin query methods 移入 `coding_session/session_view.rs`，减少 `mod.rs` owner 集中度并保持 stable facade 不变。
- `927a04b`：将 `run_sync_operation` read-only dispatcher 移入 `coding_session/operation_dispatch.rs`，同步 capability-aware plugin command 与 scheduler admission guards。
- `d69d14a`：扩展 intent-router admission 源码 guard，使 canonical dispatcher 计数覆盖独立 `operation_dispatch.rs`。
- `482d883`：将 `run_sync_mut_operation` 迁入 `coding_session/operation_dispatch.rs`，同步 navigation snapshot publication 源码守卫；fork、switch active leaf、default profile mutation 继续共享 canonical scheduler admission。
- `e39a7ac`：将 `run_operation` async dispatcher 移入 `coding_session/operation_dispatch.rs`，同步跨模块 child-lineage guard；prompt、compaction、plugin load、branch summary、self-healing、invocation/team 与 delegation approval 继续共享 frozen admission snapshot。
- `a7362c2`：将 hydration、product subscription、shutdown drain、snapshot、client connection、projection refresh 与 retained-event query 移入 `coding_session/session_connection.rs`；同步 adapter inventory、event ownership 与 startup recovery ordering guards。
- `b4494ad`：将 create/open/open-or-create/non-persistent、list/hydrate/tree/clone/fork/export 及 persistent/transient initialization 移入 `coding_session/session_lifecycle.rs`；stable lifecycle facade 与 initialization projection/event ordering 保持不变。
- `4aa69d4`：将 canonical `run`、submission lease lifecycle、commit/terminal association/drop guard 移入 `coding_session/operation_submission.rs`；API 与 intent-router 源码 guards 改为读取实际 owner，三种 dispatcher 路由保持唯一。
- `61d6deb`：将 prompt-control facade 与 exact operation-id/channel-generation cleanup guard 移入 `coding_session/session_control.rs`；dispatcher bind/cleanup 与 control ownership 合同保持不变。
- `aec0aa7`：将 operation admission、dynamic delegation-approval kind、operation ID、capability snapshot input、cwd/persistence 与 runtime/profile/plugin/delegation tool resolution 移入 `coding_session/operation_admission.rs`；dispatcher 只消费 frozen admission result。
- `bf300d7`：将 `coding_session/mod.rs` 中 5115 行内联测试迁入独立 `session_tests.rs`，以文件内显式 `#[cfg(test)] mod tests` 保证 production source scans 剥离全部测试调用；主 owner 降至 867 行。
- `d927360`：将 session-store failure injection、pending delegation fixture、persistent-session 与 capability-generation test accessors 移入 `session_test_support.rs`；源码 guard 改为检查实际 test-only owner，主 owner 降至 768 行。
- `0fec62c`：将 prompt profile application、persistent/transient context preparation、Flow execution、authorized delegation folding、transaction finalization 与 session-write metadata application 移入 `prompt_execution.rs`；event source guards 改为验证实际 owner，主 owner 降至 478 行。
- `bf5c7c7`：将 export、delegation approval、branch-summary admitted wrapper、plugin-load post-processing、agent/team invocation 与 self-healing model-repair policy 直接移入各自既有 flow/service owner；主 facade 不再持有 operation execution method 并降至 282 行，provider guard 分别锁定 ordinary/approval scoped runtime installation。
- `92a0920`：将 plugin/profile/runtime construction options、session/default cwd、default profile 与 replay-derived pending/recovery state 移入 `session_lifecycle.rs`；主 module 降至 201 行，仅保留 module graph、facade exports/import aliases 与 `CodingAgentSession` state layout。
- `f36af38`：移除 `event_service.rs` file-level dead-code suppression；test-only channel capacity、backpressure projection、unscoped recovery helper、agent-event mapper/team-abort emitter 与 recovery observation fields 改为精确 `cfg(test)`，production compile 无 event-service dead-code warning。
- `4b52c50`：移除 `plugin_load_flow.rs` file-level dead-code suppression；fixture candidate builder、outcome/service observer 与 node-id ledger 精确限制为 `cfg(test)`，删除无消费者的 plugin-load `run_with_options` 分支。
- `dde44da`：移除 `branch_summary_flow.rs` file-level dead-code suppression；outcome/pending-event observers 与 node-id ledger 精确限制为 `cfg(test)`，删除无消费者的 branch-summary `run_with_options` 分支。
- `f8b86af`：移除 `manual_compaction_flow.rs` file-level dead-code suppression；测试构造器、结果 observers、node-id ledger 与非取消 `run` 精确限制为 `cfg(test)`，保留生产使用的 cancellation-aware `run_with_options`，并删除连测试也未消费的 `final_message` accessor。
- `163928f`：移除 `agent_invocation_flow.rs` file-level dead-code suppression；node-id ledger 精确限制为 `cfg(test)`，删除无消费者的 `run_with_options` 与 child-operation-id accessor，保留 canonical invocation `run` 和内部 child operation identity 状态。
- `3645313`：移除 `prompt_flow.rs` file-level dead-code suppression；node-id ledger 与 cancellation/max-step test execution seam 精确限制为 `cfg(test)`，production graph 继续只暴露 canonical `run`。
- `68c210d`：移除 `self_healing_edit_flow.rs` file-level dead-code suppression；execution-env check/edit adapters、check-runner injection 与 diagnostics observer 精确限制为 `cfg(test)`，保留 production `EditOperations`、默认 check runner 和工具调用路径。
- `ac4a047`：移除 `flow_service.rs` file-level dead-code suppression；production 编译未暴露本地 dead code，证明当前 service/graph runners 均有真实消费者，无需引入 item-level例外。
- `3208364`：移除 `client_projection.rs` file-level dead-code suppression，并沿消费者链删除已被 SnapshotCoordinator/public connection 替代的内部 `ClientConnection`、`SubmittedOperation`、legacy `connect_client`、两个无消费者 ClientService pass-through 及 6 个只验证死表面的测试；`set_prompt_draft` fixture seam 精确限制为 `cfg(test)`。
- `ec447f7`：移除 `session_log/mod.rs` file-level dead-code suppression；event/manifest fixture builders、replay status/store path observers、explicit-cancellation 与 in-doubt assertions 精确限制为 `cfg(test)`，production durable schema、append/read、replay fold、commit/abort 路径保持可达。
- `141b184`：移除最后一个 `prompt.rs` file-level dead-code suppression；删除生产热路径上仅供单测读取、会重复保存 agent/coding events 的 `AgentRunObservation` 平行状态，删除无消费者 constructor/transaction accessor，其余 prompt context fixture observers 精确限制为 `cfg(test)`。`pi-coding-agent/src` file-level dead-code suppression 数量降为 `0`。
- `b0dcf46`：依据 `docs/architecture.md` 的 Non-Goal 决策，删除无运行消费者、无 manifest/Lua loader consumer 的 arbitrary plugin Flow extension surface：trait、registration host、extension-point enum、registry storage、capability counters、PluginService collector/panic wrapper 与 2 个模型自证测试；新增 source boundary guard 禁止该 surface 回归。
- `3033355`：移除 plugin registry/service 的 23 个 item-level dead-code suppression；编译证明 tool/command/UI/keybind registration 与 collection 都有 production consumer。删除仅测试消费的 raw hook collector，测试改为验证 canonical `run_prompt_hook`，Lua hook 测试不再窥视 raw registration。

M1 已完成。`CodingAgentSession`、CLI、print/JSON、RPC、interactive、delegation approval 和 product Flow fixtures 均显式使用 scoped `AiClient`；仓内不再读写 deprecated global provider registry；`pi-ai::registry`、`pi-agent-core` 的主要 runtime/support 模块已不再是外部模块入口；`pi-coding-agent` root deprecated re-export 已删除。M2/WP2.2 已建立 scheduler 核心并完成 prompt/compact/async canonical dispatch migration，且移除了第二 operation admission 入口；`f31ede0` 增加了禁止 scheduler admission 绕行的 product boundary guard。下一步按 workflow、invocation/delegation、plugin/runtime-write、session-navigation 顺序迁移其余 vertical slices，删除 adapter/service 层散落的 admission 判断。

本阶段验证：

- `cargo fmt --all --check`
- `cargo clippy -p pi-coding-agent --all-targets --all-features`（通过，保留既有 warnings）
- `cargo test -p pi-coding-agent --tests --no-fail-fast`
- `git diff --check`
- product provider registry boundary guards：源码和测试中的 global registry mutation caller 为 `0`
- `cargo test -p pi-ai --tests --no-fail-fast`：64 个单元测试及全部集成测试通过。
- `cargo test -p pi-agent-core --test public_api --test flow --test session_wire --test session_context`：24 个测试通过。
- `cargo test -p pi-coding-agent --test interactive_sessions --test protocol_events --test product_runtime_boundary_guards`：62 个测试通过。
- `cargo test -p pi-agent-core --test resources --test sourced_resources`：12 个测试通过。
- `cargo test -p pi-agent-core --test m9_branch_proxy_shell --test session_context`：10 个测试通过。
- `cargo test -p pi-agent-core --test compaction`：12 个测试通过。
- `cargo test -p pi-agent-core --test agent_turn_flow`：23 个测试通过。
- `cargo test -p pi-coding-agent --test api_boundary_guards --test args --test protocol_args --test session_args --test interactive_args --test bin_startup`：44 个测试通过。
- `cargo test -p pi-coding-agent --lib scheduler`：4 个 scheduler admission 测试通过。
- `cargo test -p pi-coding-agent --lib intent_router`：14 个既有 admission/control 测试通过。
- `cargo test -p pi-coding-agent --lib operation`：94 个 coding-session 单元测试通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards runtime_admission_has_no_direct_operation_control_bypass -- --exact`：admission bypass boundary guard 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast`：20 个 product boundary guard 通过。
- `cargo test -p pi-coding-agent --lib operation --no-fail-fast`：96 个 coding-session operation/admission 测试通过。
- `cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow --test delegation_execution --test agent_profile_runtime --no-fail-fast`：30 个 invocation/team/delegation/profile runtime 测试通过。
- `cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow --test delegation_execution --no-fail-fast`：28 个 root/child/delegation 行为测试通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards delegated_child_flows_require_scheduler_lineage_admission -- --exact`：child scheduler lineage boundary guard 通过。
- `cargo test -p pi-coding-agent --lib session_log::store --no-fail-fast`：16 个 manifest/event decoder、durable sequence 与 store 测试通过。
- `cargo test -p pi-coding-agent --lib capability_snapshot --no-fail-fast`：10 个 capability snapshot/runtime filter 测试通过。
- `cargo test -p pi-coding-agent --test protocol_events --no-fail-fast`：13 个 typed protocol event 测试通过。
- `cargo test -p pi-coding-agent --lib public_event --no-fail-fast`：3 个 public event contract 测试通过。
- `cargo test -p pi-coding-agent --lib event_service --no-fail-fast`：27 个 event/generation 测试通过。
- `cargo test -p pi-coding-agent --test public_api --no-fail-fast`：46 个 public API/snapshot/reconnect 测试通过。
- `cargo test -p pi-coding-agent --lib public_projection --no-fail-fast`：2 个 snapshot/reconnect projection 测试通过。
- `cargo test -p pi-coding-agent --test public_api reconnect_from_cursor_validates_stream_and_snapshot_major -- --exact`：cursor stream/version validation 通过。
- `cargo test -p pi-coding-agent --test product_event_contract --test agent_invocation --test agent_team_flow --test agent_profile_session --test delegation_execution --no-fail-fast`：31 个 typed ProductEvent consumer/contract 测试通过。
- `cargo test -p pi-coding-agent --test protocol_events --test public_api --test interactive_event_bridge --no-fail-fast`：71 个 protocol/public/interactive event 测试通过。
- `cargo test -p pi-coding-agent --lib protocol::rpc --test public_api --no-fail-fast`：RPC cursor wire-shape、reconnect boundary 与 public cursor validation 测试通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast`：21 个 RPC/runtime ownership 与 facade boundary guard 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast`：全量 integration targets 通过（包含 701 个 coding-session 单元测试及所有 product integration targets）。
- `cargo test -p pi-coding-agent --lib protocol::rpc --no-fail-fast`：13 个 RPC queue/cursor/overflow boundary 测试通过，删除 replay seam 后仍保持绿色。
- `cargo check -p pi-coding-agent`：compact cancellation test-only 收敛后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast`：21 个 facade/runtime boundary guard 通过。
- `cargo test -p pi-coding-agent --lib plugin_load --no-fail-fast`：18 个 plugin-load/canonical operation 测试通过。
- `cargo test -p pi-coding-agent --lib operation --quiet`：96 个 operation/admission 测试通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：删除 session pass-through 后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo test -p pi-coding-agent --test public_api --quiet`：47 个 public snapshot/error/facade tests 通过。
- `cargo test -p pi-coding-agent --lib snapshot_coordinator --quiet`：9 个 snapshot lifecycle/recovery tests 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 boundary guards 通过。
- `cargo test -p pi-coding-agent --test public_api --quiet`：47 个 public API/query/snapshot tests 通过，验证 session view slice 拆分后的 facade 行为。
- `cargo check -p pi-coding-agent`：`session_view.rs` 模块拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：query slice 拆分及源码 guard 修正后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo test -p pi-coding-agent --lib run_sync_operation --quiet`：4 个 read-only sync dispatch tests 通过。
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet`：10 个 stable facade/dispatcher boundary tests 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：sync dispatcher 拆分与 admission guards 修正后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo test -p pi-coding-agent --lib operation --quiet`：96 个 operation/admission 测试通过，覆盖 sync-mutable dispatcher 迁移后的 canonical operation 路径。
- `cargo test -p pi-coding-agent --test agent_profile_session --quiet`、`cargo test -p pi-coding-agent --lib canonical_fork --quiet`、`cargo test -p pi-coding-agent --lib switch_active --quiet`：profile mutation、fork 与 active-leaf navigation 定向测试通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 runtime ownership/admission boundary guards 通过。
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet`：10 个 stable facade/dispatcher boundary guards 通过。
- `cargo test -p pi-coding-agent --test public_api --quiet`：47 个 public API/snapshot tests 通过，navigation projection 继续先于 session-open publication。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：sync-mutable dispatcher 拆分及源码守卫迁移后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：async dispatcher owner 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --test operation_association --quiet`：operation/event association 测试通过。
- `cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow --test delegation_execution --no-fail-fast --quiet`：28 个 invocation/team/delegation async tests 通过。
- `cargo test -p pi-coding-agent --lib compact --quiet`、`--lib plugin_load --quiet`、`--lib branch_summary --quiet`、`--lib self_healing --quiet`：共 72 个 compaction/plugin/branch/self-healing 定向测试通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 runtime ownership/admission/child-lineage guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：async dispatcher 拆分及 child-lineage guard 迁移后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`session_connection.rs` 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --test public_api --test session_boundary_guards --test event_boundary_guards --test api_boundary_guards --no-fail-fast --quiet`：91 个 public/session/event/API ownership tests 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 adapter inventory/runtime ownership guards 通过，新 connection facade 已显式分类。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：connection/snapshot/lifecycle publication facade 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`session_lifecycle.rs` 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --test session_cli --test session_args --test session_compatibility_baseline --test interactive_sessions --test protocol_sessions --no-fail-fast --quiet`：53 个 session lifecycle/compatibility/adapter tests 通过。
- `cargo test -p pi-coding-agent --test public_api --test api_boundary_guards --test provider_registry_boundary_guards --test event_boundary_guards --test product_runtime_boundary_guards --no-fail-fast --quiet`：112 个 public/facade/provider/event/runtime boundary tests 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：construction/static lifecycle facade 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`operation_submission.rs` 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --lib submission --quiet`：8 个 submission lease/commit/drop/terminal tests 通过。
- `cargo test -p pi-coding-agent --lib protocol::rpc --quiet`、`--lib operation --quiet`、`--test operation_association --quiet`：110 个 RPC/canonical operation/association tests 通过。
- `cargo test -p pi-coding-agent --test public_api submission --quiet`：3 个 public submission lifecycle tests 通过。
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet`、`--test product_runtime_boundary_guards --no-fail-fast --quiet`：31 个 facade/runtime ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：operation submission facade 拆分及源码 guard 迁移后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`session_control.rs` 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --lib prompt_control --quiet`：10 个 prompt-control ownership/generation/cleanup tests 通过。
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet`、`--test product_runtime_boundary_guards --no-fail-fast --quiet`：31 个 facade/runtime ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：prompt-control facade 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`operation_admission.rs` 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --lib capability_snapshot --quiet`、`--lib scheduler --quiet`、`--lib intent_router --quiet`：29 个 capability/admission/router tests 通过。
- `cargo test -p pi-coding-agent --test agent_profile_runtime --test delegation_execution --no-fail-fast --quiet`：20 个 profile/delegation runtime tests 通过。
- `cargo test -p pi-coding-agent --test api_boundary_guards --quiet`、`--test product_runtime_boundary_guards --no-fail-fast --quiet`：31 个 facade/runtime ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：operation admission/capability owner 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：session test-support 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --lib --no-fail-fast --quiet`：拆分后的 701 个 lib tests 通过，1 个 ignored。
- `cargo test -p pi-coding-agent --test api_boundary_guards --test session_boundary_guards --test event_boundary_guards --no-fail-fast --quiet`：44 个 API/session/event source ownership guards 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 production adapter/runtime inventory guards 通过，独立测试文件未被计入 production adapter。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：session test-support 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`session_test_support.rs` fixture bridge 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --lib --no-fail-fast --quiet`：fixture bridge 拆分后的 701 个 lib tests 通过，1 个 ignored。
- `cargo test -p pi-coding-agent --lib partial_commit --quiet`、`--lib pending_delegation --quiet`、`--lib capability_generation --quiet`：13 个 failure/delegation/generation 定向测试通过。
- `cargo test -p pi-coding-agent --test interactive_sessions --quiet`：30 个 interactive fixture consumer tests 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 guards 通过，fixture bridges 各存在一次、直接 `cfg(test)` gated 且归属独立 owner。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：fixture bridge owner 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：`prompt_execution.rs` 拆分后的 production crate 编译通过。
- `cargo test -p pi-coding-agent --lib prompt --quiet`、`--lib delegation --quiet`、`--lib branch_summary --quiet`、`--lib compact --quiet`：190 个 prompt/delegation/branch/compaction tests 通过。
- `cargo test -p pi-coding-agent --test event_boundary_guards --quiet`：22 个 event/finalization ownership guards 通过，prompt branching 与 transaction delegation checks 已迁至实际 owner。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`、`--test session_boundary_guards --quiet`：33 个 runtime/session boundary guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：prompt execution owner 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：operation helpers 归入既有 owners 后 production crate 编译通过且未新增 coding-agent warning。
- `cargo test -p pi-coding-agent --lib operation --quiet`、`--lib plugin_load --quiet`、`--lib self_healing --quiet`：132 个 operation/plugin/self-healing tests 通过。
- `cargo test -p pi-coding-agent --test agent_invocation --test agent_team_flow --test delegation_execution --no-fail-fast --quiet`：28 个 invocation/team/delegation integration tests 通过。
- `cargo test -p pi-coding-agent --test session_boundary_guards --quiet`、`--test product_runtime_boundary_guards --no-fail-fast --quiet`、`--test provider_registry_boundary_guards --quiet`：45 个 session/runtime/provider ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：operation-specific helpers 归入 flow/service owners 后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：construction/replay helpers 归入 lifecycle owner 后 production crate 编译通过且未新增 coding-agent warning。
- `cargo test -p pi-coding-agent --test session_cli --test session_compatibility_baseline --no-fail-fast --quiet`：10 个 lifecycle/compatibility tests 通过。
- `cargo test -p pi-coding-agent --lib canonical_fork --quiet`、`--lib switch_active --quiet`、`--lib startup_recovery --quiet`、`--lib self_healing --quiet`：23 个 fork/navigation/recovery/self-healing tests 通过。
- `cargo test -p pi-coding-agent --test provider_registry_boundary_guards --quiet`、`--test session_boundary_guards --quiet`：24 个 provider/session ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：lifecycle configuration/replay helper owner 拆分后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 event-service blanket suppression 后 production crate 编译通过，无 event-service dead-code warning。
- `cargo test -p pi-coding-agent --lib event_service --quiet`：27 个 event publication/recovery/backpressure tests 通过。
- `cargo test -p pi-coding-agent --test event_boundary_guards --test public_api --test protocol_events --no-fail-fast --quiet`：82 个 event/public/protocol boundary tests 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --no-fail-fast --quiet`：21 个 runtime ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：event-service suppression 收窄后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 plugin-load blanket suppression 后 production crate 编译通过，无 plugin-load dead-code warning。
- `cargo test -p pi-coding-agent --lib plugin_load --quiet`：18 个 plugin-load/canonical operation 测试通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --quiet`、`--test session_boundary_guards --quiet`：runtime/session ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：plugin-load test surface 收窄后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 branch-summary blanket suppression 后 production crate 编译通过，无 branch-summary dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib branch_summary --quiet`：16 个 branch-summary tests 通过。
- `cargo test -p pi-coding-agent --test event_boundary_guards --quiet`、`--test product_runtime_boundary_guards --quiet`、`--test session_boundary_guards --quiet`：event/runtime/session ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：branch-summary test surface 收窄后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 manual-compaction blanket suppression 后 production crate 编译通过，无 manual-compaction dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib compact --quiet`：20 个 compaction/cancellation tests 通过。
- `cargo test -p pi-coding-agent --test event_boundary_guards --quiet`、`--test product_runtime_boundary_guards --quiet`：event/runtime ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：manual-compaction test surface 收窄后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 agent-invocation blanket suppression 后 production crate 编译通过，无 agent-invocation dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib agent_invocation --quiet`：4 个 focused invocation tests 通过。
- `cargo test -p pi-coding-agent --test agent_invocation --test delegation_execution --no-fail-fast --quiet`：23 个 invocation/delegation integration tests 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test provider_registry_boundary_guards --no-fail-fast --quiet`：runtime/provider ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：agent-invocation flow surface 收窄后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 prompt-flow blanket suppression 后 production crate 编译通过，无 prompt-flow dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib prompt --quiet`：115 个 prompt/flow tests 通过，包括 cancellation/max-step seam。
- `cargo test -p pi-coding-agent --test event_boundary_guards --test provider_registry_boundary_guards --test product_runtime_boundary_guards --no-fail-fast --quiet`：55 个 event/provider/runtime ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：prompt-flow test surface 收窄后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 self-healing blanket suppression 后 production crate 编译通过，无 self-healing dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib self_healing --quiet`：18 个 self-healing flow/tool tests 通过。
- `cargo test -p pi-coding-agent --test product_runtime_boundary_guards --test provider_registry_boundary_guards --no-fail-fast --quiet`：runtime/provider ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：self-healing test adapters 隔离后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除 flow-service blanket suppression 后 production crate 编译通过，无 flow-service dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib flow_service --quiet`：28 个 Flow orchestration tests 通过。
- `cargo test -p pi-coding-agent --test event_boundary_guards --test product_runtime_boundary_guards --test session_boundary_guards --no-fail-fast --quiet`：55 个 event/runtime/session ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：flow-service blanket suppression 删除后全量 coding-agent tests 通过（701 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：删除 legacy client projection chain 后 production crate 编译通过，无 client-projection/client-service dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib client_projection --quiet`、`--lib snapshot_coordinator --quiet`：11 个 projection/coordinator focused tests 通过。
- `cargo test -p pi-coding-agent --test public_api --test product_runtime_boundary_guards --test session_boundary_guards --no-fail-fast --quiet`：80 个 public/runtime/session boundary tests 通过，method ledger 明确禁止 `connect_client` 回归。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：legacy client projection 删除后全量 coding-agent tests 通过（695 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）；减少的 6 个测试均只覆盖已删除内部模型/pass-through。
- `cargo check -p pi-coding-agent`：移除 session-log blanket suppression 后 production crate 编译通过，无 session-log dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib session_log --quiet`：51 个 durable event/manifest/store/replay/transaction tests 通过。
- `cargo test -p pi-coding-agent --test session_compatibility_baseline --test session_boundary_guards --test event_boundary_guards --no-fail-fast --quiet`：36 个 compatibility/session/event boundary tests 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：session-log test surface 收窄后全量 coding-agent tests 通过（695 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `cargo check -p pi-coding-agent`：移除最后一个 prompt blanket suppression 后 production crate 编译通过，无 `pi-coding-agent` dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib prompt --quiet`、`--lib delegation --quiet`：153 个 prompt/delegation tests 通过。
- `cargo test -p pi-coding-agent --test event_boundary_guards --test product_runtime_boundary_guards --test provider_registry_boundary_guards --no-fail-fast --quiet`：55 个 event/runtime/provider ownership guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：prompt dead surface 收窄后全量 coding-agent tests 通过（695 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。
- `rg -n '^#!\[allow\(dead_code\)\]' crates/pi-coding-agent/src`：无匹配，file-level suppression 为零。
- `cargo check -p pi-coding-agent`：删除 plugin Flow extension 后 production crate 编译通过，无新增 warning。
- `cargo test -p pi-coding-agent --lib plugin_service --quiet`、`--lib plugin_load --quiet`、`--lib capability_snapshot --quiet`：45 个 plugin/capability focused tests 通过。
- `cargo test -p pi-coding-agent --test tool_boundary_guards --test product_runtime_boundary_guards --test session_boundary_guards --no-fail-fast --quiet`：plugin capability/non-goal 与 runtime/session guards 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：plugin Flow extension 删除后全量 coding-agent tests 通过（693 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）；减少的 2 个测试只验证已删除空壳。
- `cargo check -p pi-coding-agent`：移除 plugin registry/service item-level suppressions 后 production crate 编译通过，无 plugin owner dead-code/unused warning。
- `cargo test -p pi-coding-agent --lib plugin_service --quiet`、`--lib plugin_load --quiet`、`--lib flow_service --quiet`：63 个 plugin/load/orchestration tests 通过。
- `cargo test -p pi-coding-agent --test tool_boundary_guards --test plugin_ui_boundary_guards --test product_runtime_boundary_guards --no-fail-fast --quiet`：plugin tool/UI/runtime boundary tests 通过。
- `cargo test -p pi-coding-agent --tests --no-fail-fast --quiet`：plugin registry/service surface 收窄后全量 coding-agent tests 通过（693 个 coding-session 单元测试通过，1 个 ignored，所有 integration targets 通过）。

## 激进方案决策

本计划采用 `docs/code-cleanup-strategies.md` 中的激进方案，并将三份 crate 审阅报告中的删除项纳入架构收敛，而不是单独做“按行数删代码”的清理。执行顺序固定为 replacement-first：先建立新合同和消费者，再删除旧表面。

首批激进删除范围：

- `pi-ai`：删除 `anthropic::sse` 重复实现、`transport::retry` 透传模块、serde derive 低价值测试及重复 retry 测试；在所有调用迁移后删除 global registry helpers。
- `pi-agent-core`：删除旧 monolithic agent loop、旧 provider request preparation、未接入 Flow 的兼容节点和 legacy wrapper；删除仅验证源码形状的迁移护栏，保留能表达运行时合同的测试。
- `pi-coding-agent`：删除未消费的 plugin extension 类型、平行 compatibility event、root-level deprecated re-export、重复 guard/test support 和 facade pass-through；保留仍被 session log、snapshot、plugin tool path 消费的类型。

激进方案的硬约束：

1. 任何删除必须有 replacement consumer、API/protocol diff 和回归测试；不能以 `#[allow(dead_code)]` 消失作为完成条件。
2. Rust API breaking、live protocol major `2` 和 product event v2 在同一 release train 中协调，但 durable session decoder 必须先于 writer 升级落地。
3. `0.1.x` 只保留 maintenance 分支；`0.2.0-rc.1` 后冻结支持 API 和 wire schema，不在核心 runtime 保留隐式 v1 fallback。
4. 每个删除批次必须可单独回滚；禁止把 durable schema 原地重写和 compatibility API 删除放进同一个不可逆提交。

## 1. 项目目标

本项目不是一次按行数驱动的 dead-code cleanup，而是一次允许 breaking changes 的架构收敛。最终必须同时满足：

```text
one operation runtime owner
one operation admission path
one durable Rust-native session fact model
one typed product event stream
one snapshot/projection model
thin print / JSON / RPC / interactive adapters
operation-local capability snapshots
small stable crate facades
```

具体结果：

1. `pi-ai` 只负责 provider、model、auth、transport 和 streaming；生产路径不再依赖全局 provider registry。
2. `pi-agent-core` 只保留 Flow 驱动的 agent runtime；删除旧单体 loop 和公开 legacy entrypoint。
3. `pi-coding-agent` 的 runtime-affecting 行为全部经 `CodingAgentSession::run(Operation)` 或 control/query 入口进入统一 admission。
4. typed `ProductEvent` 是所有 adapter 的唯一实时语义边界，raw `AgentEvent`、`FlowEvent` 和 flat compatibility event 不向 adapter 泄露。
5. `SessionEvent` 继续是唯一 durable truth；旧 Rust-native session log 在 `0.2.0` 中仍可读取。
6. Snapshot cursor、reconnect、retention、overflow、terminal delivery 和 multi-client ownership 形成一个版本化协议。
7. plugin、tool 和 workflow 只接收 operation-local capability handles，不接收 raw runtime/session/provider services。
8. 删除 root-level、doc-hidden 和 deprecated 兼容表面，stable facade 成为唯一支持入口。

## 2. 非目标

- 不重写 Flow engine。
- 不把产品 session、ProductEvent 或 adapter 状态下沉到 `pi-agent-core`。
- 不把 coding-agent 产品语义放入 `pi-tui`。
- 不恢复 TypeScript session JSONL 兼容层。
- 不将 live ProductEvent 全量写入 session log。
- 不允许插件注册任意 Flow node 或获取 raw service 作为迁移捷径。
- 不以删除测试行数作为成果；边界护栏只能被等价或更强机制替代。
- 不要求旧 live RPC/UI clients 与 protocol major `2` 透明兼容；不兼容连接必须明确失败。

## 3. 当前基线与已知缺口

### 3.1 已具备的基础

- `CodingAgentSession::run(CodingAgentOperation)` 已支持 async、sync read-only 和 sync mutable dispatch。
- `CodingAgentOperation` 已覆盖 prompt、compact、branch summary、self-healing edit、agent/team invocation、plugin load/command、profile mutation、delegation、session navigation 和 export。
- `public_event.rs` 已有 Session/Profile/Agent/Team/Message/Tool/Runtime/Delegation/Workflow/Diagnostic/Capability event families。
- `SnapshotCoordinator` 已包含 client generation、takeover、retention/recovery 和 runtime lifecycle 基础。
- RPC、ProductEvent 和 UI Snapshot 已有独立 `1.0` protocol family 常量及协商逻辑。
- `SessionEventEnvelope` 已有 `session_sequence: Option<u64>`；manifest schema 为 v1，event schema 为 v2。
- capability snapshot 已进入 session admission、runtime build 和部分 workflow/service 路径。
- interactive 和 RPC 的大量 workflow 已经调用 `session.run(CodingAgentOperation::...)`。

### 3.2 尚未收敛的部分

- `OperationControl` 仍主要是单 active-operation guard，不是按 admission class 决策的 scheduler。
- query、control、runtime write、session write、root/child invocation 的规则仍散布在 facade、adapter 和 services。
- `CodingAgentEvent` 仍是内部兼容入口；部分 RPC 测试和转换路径直接构造它。
- internal `ProductEvent`、public `CodingAgentProductEvent` 和 adapter event conversion 仍有平行表面。
- TUI 仍直接调用 `ui_snapshot`、`view`、hydration 和部分 session internals；client-local 与 runtime-owned state 尚未完全隔离。
- session manifest/event validator 目前只接受单一版本，尚未形成显式 decoder matrix。
- production build 仍报告 replay/client/cancellation/snapshot 相关 dead-code warning，说明部分目标能力只有建模或测试路径。
- provider 测试夹具仍大量依赖 global registry guard，阻碍 product runtime 删除 test/debug compatibility bridge。

## 4. Breaking Release 策略

### 4.1 版本选择

所有活跃 crates 从 `0.1.0` 升到 `0.2.0`。在 `0.x` 阶段，minor bump 明确表示 Rust API breaking release。

协议版本独立管理：

| Contract | 当前 | 目标 | 策略 |
|---|---:|---:|---|
| Rust crate API | `0.1.0` | `0.2.0` | 删除 deprecated/doc-hidden 兼容面 |
| RPC protocol | `1.0` | `2.0` | 显式协商；v1 client 明确拒绝或进入独立 compatibility binary |
| ProductEvent protocol | `1.0` | `2.0` | typed family、统一 terminal/durability/operation association |
| UI Snapshot protocol | `1.0` | `2.0` | 加入 stream identity、完整 cursor 和 client/runtime state 分离 |
| Session manifest | v1 | v1 或 v2 | 只在 manifest shape 必须不兼容时升级 |
| Session event | v2 | v2 或 v3 | 保留 v2 decoder；只有 durable shape 必须变化时写 v3 |

live protocol 可以 breaking，durable history 不随 crate major 粗暴失效。旧日志必须通过 versioned decoder 读入统一内存模型，默认不原地重写。

### 4.2 支持窗口

- `0.1.x` 在 `0.2.0-rc.1` 创建后进入 maintenance-only，只接受安全和数据丢失修复。
- `0.2.0` 不保留 deprecated Rust symbols；迁移通过 guide 和 compile errors 完成。
- protocol v1 不在同一 runtime 内隐式降级。若必须短期支持，使用隔离的 `pi-coding-agent-v1` compatibility binary/branch，不在核心 runtime 保留双协议分支。
- session log v1/v2 的读取支持不设与 live protocol 相同的短窗口；删除 decoder 需要独立数据保留决策。

### 4.3 Breaking change 清单

`pi-ai`：

- 删除 root global `register`、`stream_model` 和 deprecated global built-in registration。
- 删除 `providers::anthropic::sse` 与 `transport::retry` 旧公开路径。
- 将 migration-private root modules 收窄；支持 API 只从 `pi_ai::api` 导入。
- `Agent`/runtime 必须注入 scoped `AiClient` 或 `ProviderRegistry`，不再读取全局 registry。

`pi-agent-core`：

- 删除 `agent_loop` compatibility module。
- 删除旧单体 `agent_turn_flow::runtime::run_loop` 和旧 request preparation。
- 从稳定表面移除具体 Flow node/free-function 导出，只保留 `Agent`、必要 Flow/runtime contracts 和 `api` facade。
- root re-export 不再作为支持路径；下游统一迁移到 `pi_agent_core::api`。

`pi-coding-agent`：

- 删除 root-level deprecated re-export，稳定入口仅为 `pi_coding_agent::api`。
- 删除已被 `run(Operation)` 取代的 broad workflow methods 和 adapter deep-service 入口。
- 删除 `CodingAgentProductEvent.family`、`kind` 字符串字段。
- 删除 flat `CodingAgentEvent` compatibility layer，typed ProductEvent 成为唯一实时产品事件。
- Snapshot/RPC wire 升至 major `2`；旧字段和隐式 reconnect 行为不保留。
- internal service、Flow、plugin registry 和 operation dispatch 类型不得从 stable facade 导出。

## 5. 交付模型

项目使用“里程碑 + vertical slice + deletion gate”。每个里程碑必须保持 workspace 可编译；任何兼容代码只有在 replacement consumer 和边界测试完成后才删除。

分支建议：

```text
new-main                         integration branch
release/0.1                     maintenance branch, created before first breaking merge
arch/operation-runtime          scheduler/admission work
arch/event-snapshot-v2          ProductEvent and Snapshot protocol work
arch/session-log-hardening      durable log compatibility work
release/0.2-rc                  release stabilization branch
```

每个 work package 使用独立 PR。跨里程碑 PR 不允许同时修改 durable schema、live protocol 和 public API。

## 6. 里程碑与工作包

## M0. Baseline、合同和 CI 恢复

目标：得到可重复、可比较的 breaking 前基线。

### WP0.1 恢复权威合同

- 恢复 `docs/product-event-contract.md`，或将 guard 改为引用明确的唯一权威合同。
- 校对 `architecture.md`、ProductEvent contract 和本计划的术语及版本规则。
- 添加 compatibility inventory：public symbols、wire fixtures、session fixtures、adapter commands。

### WP0.2 建立 API/protocol/session snapshots

- 保存 `pi_ai::api`、`pi_agent_core::api`、`pi_coding_agent::api` 的 public API baseline。
- 为 RPC v1、ProductEvent v1、UI Snapshot v1 保存 golden fixtures。
- 保存至少三类 session fixture：event v2 无 sequence、event v2 有 sequence、包含 incomplete/partial commit 的恢复样本。
- 保存 prompt、abort、reconnect、plugin reload、fork/export 的 deterministic faux-provider event traces。

### WP0.3 恢复强制 CI

- 让 `cargo check --workspace --all-targets` 可运行。
- 将 `cargo fmt`、Clippy `-D warnings`、workspace tests 和 boundary guards 设为 required checks。
- 将 TUI smoke 设为 adapter/event/snapshot 相关 PR 的 required check。
- 输出 dead-code suppression inventory；每项带 owner、reason、removal milestone。

退出条件：全量基线绿色；所有 breaking 前合同都有 fixture 或 API snapshot；不存在“先删后补测试”。

## M1. Provider Runtime 和 `pi-agent-core` 单路径化

依赖：M0。

### WP1.1 Scoped provider runtime

状态：完成（`8e36a8a`、`377d1f5`、`6554f63`、`80d2b86`）。

- 为 `Agent`/`AgentConfig` 定义 scoped provider runtime 输入，优先复用 `AiClient`/`ProviderRegistry`。
- 将 `ProviderStreamNode` 和 `ai_runtime` 从全局 `pi_ai::stream_model` 迁到 injected runtime。
- 更新 `pi-coding-agent::RuntimeService`，由 product owner 构造并注入 provider runtime。
- 用两个并行独立 registry 测试无串扰，覆盖 auth、model lookup 和 cancellation。

### WP1.2 删除旧 agent loop

状态：完成（`0544b97`）。

- 删除旧 monolithic `runtime::run_loop`、专用 helpers 和 `PreparedProviderRequest`。
- 删除测试内复制的 `stream_options_for_turn`，改为 owning unit test 或 Agent observable test。
- 将使用 `PrepareContextNode`/`DecideStopOrToolsNode` 的集成测试迁成生产 `AgentTurnFlow` vertical slices；不为测试扩大 stable API。
- 删除 `agent_loop` wrapper 和对应只验证 legacy 隔离的测试。

### WP1.3 收窄 lower-level facade

状态：进行中。

- 下游全部迁移到 `pi_ai::api` 和 `pi_agent_core::api`。
- 删除 global registry helpers、旧 SSE/retry paths 和过度 Flow node exports。
- boundary guards 更新为验证新 facade，而不是删除 guard。

退出条件：一个 provider runtime 路径、一个 agent turn path；并发 scoped registries 无串扰；`pi-coding-agent -> pi-agent-core -> pi-ai` 依赖方向不变。

## M2. Unified Operation Admission 和 Scheduler

依赖：M1；可以与 M3 的 decoder-only 工作并行，但不得同时落同一 runtime contract PR。

### WP2.1 固化 operation contract

- 公开 `CodingAgentOperation` 的稳定字段只包含用户意图，不暴露 dispatch/service 细节。
- internal metadata 必须包含 `operation_id`、kind、origin/client、admission class、parent/root association、capability generation、runtime generation、idempotency key。
- 定义 `OperationOutcome` terminal status：completed、failed、aborted、rejected、in-doubt。
- 为每个 operation 建立 descriptor table test，禁止遗漏 class/durability/terminal family。

### WP2.2 引入真正的 scheduler

- 用 `OperationScheduler` 替代 `OperationControl` 的单 active guard。
- 实现 Query、ReadOnly、SessionWriteRoot、NonSessionRoot、RuntimeWrite、Child、Control admission classes。
- 明确 root slot、child lineage、runtime generation mutation、session write serialization 和 control priority。
- scheduler 返回 typed admission/rejection，不让 adapter 拼接 busy error string。
- cancellation token、prompt control 和 client ownership 绑定到 operation identity，不绑定到临时 task 地址。

### WP2.3 逐 vertical slice 迁移

按以下顺序迁移，每一项单独 PR：

1. Prompt + steer/follow-up/abort。
2. Compact + branch summary + self-healing edit。
3. Agent/team root invocation + delegated child invocation。
4. Plugin load/command + profile/settings runtime mutation。
5. Delegation approve/reject。
6. Fork/switch/export 和其他 session navigation/read operations。

每个 slice 同时迁移 interactive、print/JSON、RPC 和 public API tests。禁止 adapter 直接调用 `FlowService`、`SessionService`、`RuntimeService` 或 plugin services。

退出条件：所有 runtime-affecting intent 经一个 scheduler；control 在 backpressure/busy 时仍有优先级；root/child operation association 可由测试证明。

## M3. Durable SessionEvent Hardening

依赖：M0；最终接入依赖 M2 operation metadata。

### WP3.1 Versioned decoder matrix

- 将当前“版本必须等于常量”的 validator 改为显式 decoder dispatch。
- v2 event decoder 支持有/无 `session_sequence` 的历史日志。
- 若新字段可选且语义向后兼容，继续写 v2；只有 required shape 不兼容才引入 v3 writer。
- manifest 同理：能以 optional/additive 字段表达时保留 v1。
- unknown required feature fail closed；错误包含 schema、version、event id 和恢复建议，不输出敏感内容。

### WP3.2 Durable sequence 和 transaction

- 正式定义 `session_sequence` 单调性、append CAS、duplicate idempotency key 和 partial commit 语义。
- durable event 记录 operation/root/parent、capability/runtime generation 和 terminal status。
- startup scan 将 started-but-not-terminal operation 标记为 in-doubt，并写显式 recovery marker。
- fork/clone/export/replay 只从 SessionEvent/manifest 重建，不依赖 live ProductEvent 或 adapter state。

### WP3.3 故障注入验证

- 覆盖 append 前、append 后 manifest 更新前、terminal append 失败、重复重试、进程重启。
- 验证 completed/failed/aborted/recovered/in-doubt 可区分。
- 验证旧 fixture 可 replay、fork、clone、tree、export。

退出条件：旧日志可读；新日志 durable sequence 明确；partial commit 不会被误报为成功或静默丢失。

## M4. Capability Snapshot 闭环

依赖：M2 metadata；与 M3 的实现可部分并行。

### WP4.1 唯一权限语言

- admission 时冻结 `OperationCapabilitySnapshot`。
- provider/model 访问只接受 `ModelCapability`。
- filesystem/shell/tool 分别接受 narrow capability handles/set。
- plugin host 只接受 `PluginCapabilitySet`。
- durable read/write 分别接受 `SessionReadCapability`/`SessionWriteCapability`。
- 删除 operation/Flow/tool/plugin 对 raw auth、runtime、session store/service 的绕行访问。

### WP4.2 Generation 和 revocation

- RuntimeWrite 安装新 generation，不原地修改 active snapshot。
- `CapabilityChanged` 包含 generation 和 revocation policy。
- active operation 默认继续使用冻结 generation；需要强制撤销时产生显式 cancel/failure/revocation event。
- snapshot、ProductEvent 和 durable audit reference 使用同一 generation identity。

### WP4.3 清理 capability/plugin suppressions

- 移除 `capability_snapshot.rs` 文件级 `allow(dead_code)`，逐符号证明保留或删除。
- 移除 `event_service.rs`、`flow_service.rs` 及 plugin-load、branch-summary、manual-compaction、agent-invocation、prompt、self-healing Flow 文件级 suppression；测试构造/观察/取消验证/adapters 使用 `cfg(test)`，无消费者的平行执行入口直接删除或收窄。
- 移除 `client_projection.rs` 文件级 suppression；只保留 SnapshotCoordinator 和 public connection 实际消费的 snapshot/cursor/draft/identity 类型，删除平行 client/submission state model 与 legacy connector。
- 移除 `session_log/mod.rs` 文件级 suppression；durable wire/store/replay/transaction 测试 seam 使用 `cfg(test)`，不得用父模块 blanket suppression 掩盖子模块死表面。
- 移除 `prompt.rs` 文件级 suppression；禁止为测试 observation 在 production prompt context 中复制保存 agent/coding event，测试应断言 canonical coding-event/output 状态。
- 保留已有 Lua/runtime 消费者的 tool/command/hook/UI/keybind 能力。
- Flow extension 产品决策：当前不支持 arbitrary Flow nodes/subflows；仓内没有 loader/manifest/runtime consumer，已删除声明式空壳、capability counter 与测试，并以 source guard 锁定 architecture Non-Goal。未来 restricted extension point 必须以新合同重新设计。

退出条件：任一 operation 的权限决策可由 snapshot 解释；active operation 不静默热更新；plugin/tool 无 raw service capability escape。

## M5. ProductEvent v2、Snapshot v2 和 Adapter 收敛

依赖：M2、M3 terminal semantics、M4 generation semantics。

### WP5.1 唯一 typed ProductEvent

- 以现有 family enums 为基础，使 internal/public 共享一个 canonical typed model。
- 每个 event 统一带 stream identity、sequence、operation association、durability 和可选 terminal metadata。
- 建立 service/domain outcome 到 ProductEvent 的唯一 mapper。
- 禁止新增 `CodingAgentEvent` variant；先迁移 emitter，再迁移 consumer，最后删除 compatibility enum。
- 删除 deprecated `family`/`kind` string fields；wire 直接序列化 typed family/kind。

### WP5.2 Snapshot v2 contract

- cursor 至少包含 `stream_id`、last product sequence、capability generation、snapshot protocol version。
- 明确 snapshot 在 sequence N 包含的状态边界。
- 分离 client-local drafts、runtime-owned accepted/submitted operation、committed session projection。
- client generation/takeover 后旧 handle 必须失败；ack/detach/terminal receipt 有容量和幂等规则。

### WP5.3 Reconnect 和 backpressure

- retained range 覆盖 cursor 时 replay；否则返回 typed fresh-snapshot-required。
- bounded queue overflow 不伪装成普通 event；terminal/control/recovery 不能被 delta 挤掉。
- slow subscriber 明确 detach/reconnect；shutdown 有 drain boundary。
- 对 pending/live/replayed overlap 定义去重键和顺序。

### WP5.4 迁移所有 adapters

顺序：

1. print mode；
2. JSON mode；
3. RPC；
4. interactive/TUI。

每个 adapter 只能接收 Snapshot + ProductEvent + typed command outcome。TUI 不再直接调用 session hydration/runtime internals；RPC 不再构造 flat compatibility events。

退出条件：四类 adapter 的同一操作产生语义等价 trace；gap/reconnect/multi-client 测试通过；raw Flow/Agent/internal event 不出现在 protocol。

## M6. Public Facade 删除和模块拆分

依赖：M1--M5 全部退出条件。

### WP6.1 删除 compatibility APIs

- 执行第 4.3 节完整 breaking 清单。
- stable import fixture 只使用三个 crate 的 `api` facade。
- compile-fail fixture 验证 internal operation dispatch、services、Flow nodes、plugin registry 无法从 facade 导入。
- 生成 `0.1 -> 0.2` public API diff，并逐项关联 migration guide。

### WP6.2 拆分 `CodingAgentSession`

- 按 operation dispatch、connection/snapshot、query/view、lifecycle 和 test support 移动 impl。
- operation-specific business logic 放入 operation/service modules。
- facade delegation 保持显式，不使用宏掩盖 owner 或 side effect。
- 删除 replacement 已完成的 pass-through、test seam 和 file-level suppression。

### WP6.3 测试基础设施收敛

- 重复 EnvGuard/ProviderGuard 合并为一个 test-support owner。
- 手写 rustc negative runner 可迁到 `trybuild`，但失败能力必须等价。
- 源码扫描规则只有在 visibility、lint 或 CI check 接管后才能删除。
- wire、durability、association、ordering 和 boundary tests 保留。

退出条件：deprecated symbols 和 file-level dead-code suppression 为零；stable facade 小且可快照；无隐藏 fallback path。

## M7. `0.2.0` Release Train

依赖：M6。

### WP7.1 Alpha

- 升 crate versions 到 `0.2.0-alpha.1`，protocol constants 到 `2.0`。
- 发布 API migration guide、RPC/ProductEvent/Snapshot v2 spec、session compatibility matrix。
- 对实际 `0.1` session fixture 做只读 replay、fork、export soak test。

### WP7.2 Release Candidate

- `0.2.0-rc.1` 后冻结 public API、protocol schema 和 durable writer shape。
- 只接受 correctness、data loss、security、protocol ambiguity 和 migration blocker 修复。
- 运行长时 reconnect/backpressure、parallel clients、abort/retry 和 interrupted commit 测试。

### WP7.3 General Availability

- 发布 `0.2.0` 和完整 breaking changelog。
- 创建 `release/0.2`；保留 `release/0.1` maintenance policy。
- 发布后首个 patch 只处理兼容 decoder、数据恢复和明确回归，不恢复已删除 architecture path。

退出条件：所有 release gates 绿色；migration guide 覆盖每个 public API deletion；没有 P0/P1 未决数据或协议问题。

## 7. 依赖关系和关键路径

```text
M0 Baseline
  |
  +--> M1 Scoped provider + single agent loop
  |       |
  |       +--> M2 Operation scheduler/admission
  |               |
  |               +--> M4 Capability snapshots
  |
  +--> M3 SessionEvent decoder/hardening

M2 + M3 + M4
  |
  +--> M5 ProductEvent/Snapshot/Adapters v2
          |
          +--> M6 Facade deletion/module cleanup
                  |
                  +--> M7 0.2 release train
```

关键路径是 `M0 -> M1 -> M2 -> M4 -> M5 -> M6 -> M7`。M3 可以提前进行 decoder 和 fixture 工作，但 terminal/generation 字段接入必须等待 M2/M4 合同稳定。

## 8. PR 序列

建议的最小 review 单元如下：

1. 恢复 ProductEvent contract 和 all-targets baseline。
2. 添加 public API、protocol v1、session v1/v2 fixtures。
3. 注入 scoped `AiClient`/`ProviderRegistry`。
4. 删除 global provider runtime 调用。
5. 删除旧 monolithic agent loop。
6. 收窄 `pi-ai`/`pi-agent-core` facade。
7. 固化 operation descriptor/metadata table。
8. 引入 scheduler core 和 admission result。
9. 迁移 prompt/control slice。
10. 迁移 workflow slice。
11. 迁移 invocation/delegation slice。
12. 迁移 plugin/runtime-write/session-navigation slice。
13. 引入 versioned session decoder。
14. durable sequence/idempotency/recovery hardening。
15. capability handle 全路径接入。
16. canonical typed ProductEvent。
17. Snapshot v2 cursor/client state。
18. reconnect/backpressure hardening。
19. 迁移 print/JSON adapters。
20. 迁移 RPC v2 adapter。
21. 迁移 interactive/TUI adapter。
22. 删除 `CodingAgentEvent` 和旧 wire fields。
23. 删除 root/deprecated/doc-hidden compatibility surface。
24. 拆分 `CodingAgentSession` 和收敛 test support。
25. 版本升级、migration guide 和 `0.2.0-alpha.1`。

PR 8--12、16--21 必须附带 event-sequence/association tests；PR 13--14 必须附带旧 fixture 和故障注入 tests；PR 22--23 必须附带 API/protocol diff。

## 9. 验收矩阵

### 9.1 每个 PR

```bash
cargo fmt --all --check
cargo check -p <owning-crate>
cargo test -p <owning-crate> <focused-test-or-target>
git diff --check
```

### 9.2 每个里程碑

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

adapter、snapshot、event 或 terminal lifecycle 变更额外执行：

```bash
scripts/tui-smoke.sh
```

### 9.3 Release candidate 强制场景

| 场景 | 必须证明 |
|---|---|
| Prompt streaming | delta 顺序、terminal 唯一、operation ID 一致 |
| Abort/control | busy/backpressure 下可达，target identity 正确 |
| Root/child invocation | scheduler 不误拒绝，lineage 可追踪 |
| Plugin reload | generation 更新，active operation 不热变权限 |
| Reconnect in retention | 无重复、无缺口、cursor 前进 |
| Reconnect after eviction | 明确要求 fresh snapshot |
| Slow client | bounded memory，terminal/recovery 不被挤掉 |
| Multi-client takeover | stale handle 失败，client-local draft 隔离 |
| Partial commit | success 不误报，重启后 in-doubt/recovered 明确 |
| Old session logs | replay/fork/clone/tree/export 兼容 |
| Machine-readable modes | stdout/JSONL 无诊断污染 |
| Terminal lifecycle | resize/cursor/cleanup/scrollback 正常 |

## 10. Release Gates

### Gate A：允许开始删除 compatibility event/API

- 所有 production consumers 已迁移；
- public API/protocol diff 已生成；
- replacement tests 绿色；
- `rg`/CodeGraph 无 legacy caller；
- migration guide 已有对应条目。

### Gate B：允许升级 live protocol major

- v2 spec 和 fixtures 完整；
- negotiation 对 v1 明确失败；
- Snapshot + ProductEvent 可重建 adapter state；
- gap、overflow、terminal ordering 和 multi-client 行为均有测试。

### Gate C：允许升级 durable writer

- 旧 decoder 和 fixture tests 已存在；
- 不升级版本无法表达 required invariant；
- migration/read strategy 经过 recovery review；
- 不执行不可逆的原地批量重写。

### Gate D：允许发布 `0.2.0`

- workspace/Clippy/tests/TUI smoke 全绿；
- zero deprecated use、zero unexpected dead-code warning；
- old session compatibility matrix 全绿；
- release notes、migration guide、protocol specs 完整；
- 无 P0/P1 未解决问题。

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| scheduler 重写改变并发语义 | operation 被错误拒绝或并发写入 | admission table tests；逐 vertical slice 迁移 |
| event 迁移丢 terminal/association | UI 卡住、RPC 误报完成 | normalized terminal contract；adapter trace differential tests |
| snapshot cursor 定义不一致 | reconnect 重复或丢事件 | 明确 sequence boundary；retention/eviction property tests |
| durable schema 升级破坏旧日志 | 历史不可恢复 | versioned decoder；fixture corpus；禁止默认原地重写 |
| capability 收敛过度授权 | plugin/tool 越权 | narrow handles；negative capability tests；generation audit |
| 同时删除测试和实现 | 回归不可见 | replacement-first gate；禁止同 PR 无替代删护栏 |
| facade 大幅删除遗漏仓外调用 | 用户升级困难 | API diff；migration guide；alpha/RC feedback window |
| TUI 迁移引入生命周期回归 | 终端状态损坏 | virtual terminal tests + `scripts/tui-smoke.sh` |
| 双协议兼容污染新 runtime | 长期分支和隐式降级 | v1 独立 compatibility branch/binary；核心只实现 v2 |

## 12. 回滚策略

- M1--M4 的每个 vertical slice 可以按 PR revert，不能依赖数据库式不可逆 migration。
- live protocol v2 在 alpha/RC 阶段可以整体回退到上一个 RC；禁止在同一 protocol major 下改变既有字段语义。
- durable writer 在 RC 前默认保持当前版本；若启用新 writer，必须先发布支持新旧 decoder 的版本。
- 已写入的新 SessionEvent 不通过 history rewrite 回滚；回滚版本必须能读取它，或在发布前阻止 writer 上线。
- compatibility API 一旦在 GA 删除，不通过 patch release 恢复平行 architecture；只修 migration blockers 或提供外部 adapter。

## 13. 项目追踪指标

主指标：

- runtime-affecting adapter bypass 数量：目标 `0`；
- production global provider registry caller 数量：目标 `0`；
- flat compatibility event emitter/consumer 数量：目标 `0`；
- deprecated symbol/use 数量：目标 `0`；
- unexpected dead-code warning 和 file-level suppression 数量：目标 `0`；
- old session fixture replay success：目标 `100%`；
- protocol gap/overflow/terminal scenarios 覆盖率：目标 `100%` 场景覆盖；
- boundary guard 等价替代率：目标 `100%`，不得降低失败能力。

不作为主指标：净删除行数。替代合同、decoder 和 failure tests 可能增加代码；项目成功由路径唯一性、合同清晰度和恢复正确性衡量。

## 14. Definition of Done

当且仅当以下条件同时成立，architecture convergence 才完成：

1. `pi-ai`、`pi-agent-core`、`pi-coding-agent` 只有各自 `api` facade 是支持表面。
2. scoped provider runtime 替代所有 global registry production path。
3. `AgentTurnFlow` 是唯一 agent turn 实现。
4. 所有 runtime-affecting intent 经统一 scheduler/admission。
5. typed ProductEvent v2 是唯一 adapter event boundary。
6. Snapshot v2 支持明确的 multi-client、cursor、gap 和 backpressure 语义。
7. SessionEvent 是唯一 durable truth，旧日志仍可读。
8. operation-local capability snapshot 是唯一权限语言。
9. deprecated/legacy paths 已通过明确 deletion commits 删除，没有 fallback。
10. `0.2.0` release gates 全部通过，迁移文档可让 `0.1` 使用者完成升级。
