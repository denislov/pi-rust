---
phase: 02-canonical-facade-correctness
reviewed: 2026-07-11T08:15:46Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - crates/pi-coding-agent/src/lib.rs
  - crates/pi-coding-agent/src/coding_session/public_operation.rs
  - crates/pi-coding-agent/src/coding_session/mod.rs
  - crates/pi-coding-agent/src/coding_session/profiles.rs
  - crates/pi-coding-agent/src/coding_session/session_service.rs
  - crates/pi-coding-agent/tests/public_api.rs
  - crates/pi-coding-agent/tests/api_boundary_guards.rs
  - crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs
findings:
  critical: 0
  warning: 4
  info: 0
  total: 4
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-07-11T08:15:46Z  
**Depth:** standard  
**Files Reviewed:** 8  
**Status:** issues_found

## Summary

本次审查覆盖稳定 facade、public/private operation 映射、canonical dispatcher、高风险持久化 mutation，以及新增的结构性 boundary guards。当前生产路径中的 `CodingAgentSession::run` 分派和 delegation manifest 失败后的 `PartialCommit` 映射没有发现可证明的行为回归；问题集中在边界测试的解析与扫描策略。现有测试全部通过，但以下 4 个漏检点允许后续改动绕过 Phase 02 声称的 facade/fault-control 封闭保证。

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: 稳定 API 隐私守卫无法识别 glob re-export

**File:** `crates/pi-coding-agent/tests/api_boundary_guards.rs:184-210`  
**Issue:** 测试只把 `api` 模块文本拆成 identifier 集合，再查找禁用类型的名字。若改成 `pub use crate::coding_session::*;`，集合中只出现 `coding_session`，不会出现 `ProfileRegistry` 或 `ProfileRegistryOptions`，但这两个当前为 `pub` 的实现类型会被实际重新导出。该改动因此能通过 `stable_api_excludes_internal_runtime_contracts`，直接破坏 FACADE-04 的隐私保证。  
**Fix:** 解析 `api` 模块中的 `use` tree，并无条件拒绝 glob re-export；随后对每个显式 re-export 的最终 identifier 应用 denylist。更稳妥的实现是使用 `syn::parse_file`/`syn::ItemUse`，并保留 downstream compile/API snapshot 作为正向证据。

### WR-02: 通用 fault-control 扫描完全跳过最关键的 store owner 文件

**File:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs:82-83`  
**Issue:** 发现 `path == store_path` 后直接 `continue`，导致通用的 injection/failure/fault vocabulary 扫描不检查 `session_log/store.rs`。因此，在该文件中新增未加 `#[cfg(test)]` 的 `FailureHook`、`FaultPoint` 或 `inject_failure` 等生产控制点时，五个已知签名和七个已知调用计数仍可保持不变，测试也会通过。这正是该 guard 按计划必须拒绝的新增生产 fault surface。  
**Fix:** 不要跳过整个 `store.rs`。对所有文件运行通用 vocabulary 扫描，只对已经通过精确计数和直接 `#[cfg(test)]` 校验的已知定义/调用做逐项豁免；任何其他匹配项都必须继续执行 item/module test-gate 判定。

### WR-03: Rust sanitizer 把 lifetime 当作 char literal，可能吞掉后续源码

**File:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs:842-845`  
**Issue:** sanitizer 遇到任意 `'` 都进入 `State::Char`，但 Rust 的 `&'static str`、`Formatter<'_>` 和 `impl<'a>` 是 lifetime，不是字符字面量。当前被扫描源码已经包含这些形式，例如 `session_log/store.rs:60` 和 `:66`。sanitizer 会一直擦除到下一个 apostrophe，导致 brace depth、method discovery、fault identifier 和 trait/module 扫描基于被破坏的 token stream，产生取决于后续 apostrophe 配对的静默漏检。  
**Fix:** 使用 Rust parser/tokenizer 代替手写 sanitizer，优先采用 `syn::parse_file`。若暂时保留 lexer，必须区分 lifetime/label 与合法 char literal，并为 `&'static str`、`Formatter<'_>`、`impl<'a>`、byte/raw strings、嵌套注释添加直接单元测试。

### WR-04: alternate trait facade 检查只查看 trait 声明行

**File:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs:662-668`  
**Issue:** `public_trait` 命中后，代码仅用 `trimmed.contains("run")` 检查声明所在的单行。常规多行 Rust trait，例如 `pub trait OperationRunner {` 后在下一行声明 `fn run(...)`，不会被报告；文件级 `source.contains("CodingAgentSession")` 也没有验证 trait 的参数、返回值或 impl 是否真正转发 canonical contracts。这样可以增加计划明确禁止的 trait-based alternate facade 而保持 boundary test 绿色。  
**Fix:** 解析完整 `ItemTrait` body，检查公开/`pub(crate)` trait 中的方法签名是否接收或返回 `CodingAgentSession`、`CodingAgentOperation`、`CodingAgentOperationOutcome`，并检查相关 trait impl/forwarder；至少也应先提取平衡的 trait body，而不是只检查声明行。

---

_Reviewed: 2026-07-11T08:15:46Z_  
_Reviewer: the agent (gsd-code-reviewer, generic-agent workaround)_  
_Depth: standard_
