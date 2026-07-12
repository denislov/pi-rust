# Phase 4: Test Convergence and Compatibility Deletion - Pattern Map

**Mapped:** 2026-07-13
**Files analyzed:** 8 migration/guard files plus the canonical operation implementation and contracts
**Analogs found:** 8 / 8

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/pi-coding-agent/src/coding_session/mod.rs` | service/facade, owner tests and deletion targets | request-response, event-driven, file I/O | `run` and `run_sync_*_operation` in the same file | exact |
| `crates/pi-coding-agent/src/coding_session/public_operation.rs` | model/contract | transform, request-response | `CodingAgentOperation` and `CodingAgentOperationOutcome` in the same file | exact |
| `crates/pi-coding-agent/tests/agent_invocation.rs` | integration test | request-response, streaming, file I/O | canonical `run` outcome pattern | exact |
| `crates/pi-coding-agent/tests/agent_team_flow.rs` | integration test | request-response, streaming, file I/O | canonical `run` outcome pattern | exact |
| `crates/pi-coding-agent/tests/agent_profile_runtime.rs` | integration test | request-response, streaming | canonical `Prompt` operation path | exact |
| `crates/pi-coding-agent/tests/agent_profile_session.rs` | integration test | request-response, file I/O, replay | `SetDefaultAgentProfile` operation path | exact |
| `crates/pi-coding-agent/tests/delegation_execution.rs` | integration test | event-driven, file I/O, request-response | `ApproveDelegation` / `RejectDelegation` outcome paths | exact |
| `crates/pi-coding-agent/tests/public_api.rs` | public API integration test | request-response, file I/O | `pi_coding_agent::api` facade tests | exact |
| `crates/pi-coding-agent/tests/support/mod.rs` | test utility/fixture | transform only | pure typed outcome extractors and existing deterministic fixtures | role-match |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | source/API guard test | batch transform/source audit | current compatibility method ledger | exact |
| `crates/pi-coding-agent/tests/api_boundary_guards.rs` | public API guard test | batch/source audit | stable `api` facade restrictions | role-match |

## Pattern Assignments

### `crates/pi-coding-agent/src/coding_session/mod.rs` (service/facade, request-response/event-driven)

**Analog:** the existing canonical `CodingAgentSession::run` and private dispatchers at `mod.rs:248-260`, `1310-1449`.

**Canonical facade pattern** (`mod.rs:248-260`):

```rust
pub async fn run(
    &mut self,
    operation: CodingAgentOperation,
) -> Result<CodingAgentOperationOutcome, CodingSessionError> {
    let operation = operation.into_internal(self.default_plugin_load_options.clone());
    let dispatch_mode = operation.metadata().dispatch_mode;
    let outcome = match dispatch_mode {
        OperationDispatchMode::Async => self.run_operation(operation).await?,
        OperationDispatchMode::SyncReadOnly => self.run_sync_operation(operation)?,
        OperationDispatchMode::SyncMutable => self.run_sync_mut_operation(operation)?,
    };
    Ok(CodingAgentOperationOutcome::from_internal(outcome))
}
```

Migrate owner workflow tests to call this method. Do not add a test helper that accepts a session and chooses an operation internally: the operation must remain visible at each call site. Preserve the existing event, replay, transaction, and typed error assertions after matching the returned outcome.

**Owner-private exception pattern:** co-located tests may continue to use crate-private operation/dispatcher paths only when testing custom `PluginLoadOptions`, metadata/dispatch classification, or action-specific fault injection that the public enum deliberately cannot express. Put a short reason beside each exception. The existing owner test imports at `mod.rs:2305-2334` show the appropriate location for FauxProvider, session-log, and `StoreFailurePoint` fixtures; do not move those controls into `api`.

**Deletion order:** for each method group, first receiver-aware audit all production and test calls, run focused behavior tests plus guards and `cargo check -p pi-coding-agent`, then delete the definition. Do not replace a deleted method with a renamed wrapper. Keep `create`, `open`, `open_or_create`, `non_persistent`, `list`, lifecycle/query/control APIs, and static repository helpers.

### `crates/pi-coding-agent/src/coding_session/public_operation.rs` (contract, transform/request-response)

**Analog:** `CodingAgentOperation` and `CodingAgentOperationOutcome` at `public_operation.rs:42-104`.

**Operation vocabulary** (`public_operation.rs:42-83`):

```rust
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    BranchSummary { options: PromptTurnOptions, source_leaf_id: String,
        target_leaf_id: String, custom_instructions: Option<String>,
        reuse: BranchSummaryReusePolicy },
    SelfHealingEdit(SelfHealingEditRequest),
    InvokeAgent(AgentInvocationOptions),
    InvokeTeam(AgentTeamOptions),
    PluginLoad,
    SetDefaultAgentProfile { profile_id: ProfileId },
    ApproveDelegation { operation_id: String, tool_call_id: String },
    RejectDelegation { operation_id: String, tool_call_id: String,
        reason: String },
    ForkSession { target_leaf_id: Option<String> },
    SwitchActiveLeaf { target_leaf_id: String },
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}
```

Use the stable `pi_coding_agent::api` re-exports in integration tests. The operation conversion at `public_operation.rs:106-157` is the source of truth for mapping profile/delegation/export variants to internal operations; tests should not construct `Operation` directly outside owner-private exceptions.

**Exact typed outcome extraction** (`public_operation.rs:86-104`, with conversion at `160-180`):

```rust
let outcome = session
    .run(CodingAgentOperation::InvokeAgent(options))
    .await
    .expect("invoke agent through canonical facade");
let invocation = match outcome {
    CodingAgentOperationOutcome::AgentInvocation(outcome) => outcome,
    _ => unreachable!("invoke-agent operation returned another outcome"),
};
```

Apply the same exact variant rule for `Prompt`, `Compact`, `BranchSummary`, `SelfHealingEdit`, `AgentTeam`, `PluginLoad`, `DefaultAgentProfileChanged`, `DelegationApproved`, `DelegationRejected`, `Export`, and `ExportHtml`. Keep exhaustive matching local unless repetition is meaningful enough for a pure extractor.

### Integration workflow suites (request-response/streaming/file I/O)

**Files:** `tests/agent_invocation.rs`, `tests/agent_team_flow.rs`, `tests/agent_profile_runtime.rs`, `tests/agent_profile_session.rs`, `tests/delegation_execution.rs`, `tests/public_api.rs`.

**Analog:** the public facade pattern above and the existing deterministic test architecture described in `04-RESEARCH.md:194-207`.

Migration rules:

1. Import operations, outcomes, session options, and result types from `pi_coding_agent::api` where exported.
2. Make tests async when migrating formerly synchronous workflow methods; call `session.run(...).await` even for sync-dispatched operations.
3. Match the exact outcome variant, then retain every substantive assertion: agent/team ordering, profile/provider/tool behavior, output, ProductEvents, pending delegation state, transaction/operation IDs, replay/reopen state, export paths, branch summaries, and structured `PartialCommit` errors.
4. Keep deterministic FauxProvider and tempfile/session fixtures. A successful outcome alone is not a replacement for durability or event assertions.

Family assignments:

- `agent_invocation.rs`: `InvokeAgent` and `ExportCurrent`; preserve invocation output/profile validation, event stream, replay, and export checks.
- `agent_team_flow.rs`: `InvokeTeam` and `ExportCurrent`; preserve member ordering, state, and export assertions.
- `agent_profile_runtime.rs`: `Prompt`; preserve profile/provider/tool/delegation runtime behavior.
- `agent_profile_session.rs`: `SetDefaultAgentProfile`; preserve persisted and reopened profile state.
- `delegation_execution.rs`: `Prompt`, `ExportCurrent`, `ApproveDelegation`, and `RejectDelegation`; preserve pending queue, execution, durable event, replay, exact error, and `PartialCommit` checks.
- `public_api.rs`: public operations for prompt, summary, self-healing, profile mutation, and delegation; replace old-method compile assertions with positive canonical operation checks and absence evidence.

### `crates/pi-coding-agent/tests/support/mod.rs` (test utility, transform only)

**Analog:** existing reusable integration fixtures in this module. Add only pure typed outcome extraction when multiple public suites repeat the same match.

Allowed shape:

```rust
fn extract_agent_invocation(
    outcome: CodingAgentOperationOutcome,
) -> AgentInvocationOutcome {
    match outcome {
        CodingAgentOperationOutcome::AgentInvocation(value) => value,
        other => panic!("expected agent invocation outcome, got {other:?}"),
    }
}
```

The helper accepts an outcome and returns a typed payload. It must not select `CodingAgentOperation`, create or own a session, run operations, hide errors, hold runtime services, or recreate any deleted broad facade. Keep custom options, metadata, dispatcher, and fault fixtures in co-located `coding_session` tests.

### `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` (guard, batch/source audit)

**Analog:** the existing `canonical_operation_facade_has_no_new_workflow_wrappers` ledger and the receiver-aware source checks described in `04-RESEARCH.md:262-265`.

Convert the old “compatibility methods exist exactly once” ledger into two explicit ledgers:

- **Retained contract ledger:** `run`, construction/open/resume, `list`, `export_session_html`, subscriptions, snapshots, connect/capabilities/view, queries, control, plugin UI/query helpers, hydration/tree/clone/static fork helpers, and other non-operation lifecycle/query APIs remain present.
- **Absent old-operation ledger:** old definitions, old receiver calls, and synonymous compatibility wrappers for each deleted group are absent. Check method/receiver shape rather than globally banning bare names, because `InteractiveRoot`, `SessionService`, `Agent`, and static repository helpers legitimately share names.

Run this guard before and after each deletion group. Keep alternate-facade detection active so a renamed compatibility wrapper fails the guard.

### `crates/pi-coding-agent/tests/api_boundary_guards.rs` (public API guard, batch/source audit)

**Analog:** existing stable facade restrictions. Public workflow compile contracts should use `pi_coding_agent::api::{CodingAgentOperation, CodingAgentOperationOutcome, CodingAgentSession, ...}` and stop compiling `set_default_agent_profile_id`, `approve_delegation_confirmation`, and `reject_delegation_confirmation` after migration. Preserve checks for the intended stable lifecycle/query/control surface.

### Owner-private custom options and failure paths (service/test, event-driven/file I/O)

**Analog:** `PluginLoadContext` at `plugin_load_flow.rs:213-264` and test-only session failure controls in `session_service.rs:805-811`.

`PluginLoad` intentionally has no public `PluginLoadOptions` payload. Preserve `load_plugins(PluginLoadOptions)` only for owner tests whose custom options cannot be represented by the public operation, and document that reason at each call. Delete convenience-only calls. Fault injection remains direct `#[cfg(test)]`, crate-private, and action-specific; never expose generic selectors, internal services, queues, or registries.

## Shared Patterns

### Stable Public Boundary
**Source:** `crates/pi-coding-agent/src/coding_session/mod.rs:248-260`, `crates/pi-coding-agent/src/coding_session/public_operation.rs:42-180`
**Apply to:** all public behavior/integration tests.

Construct a typed `CodingAgentOperation`, call `session.run(...).await`, and match the exact `CodingAgentOperationOutcome` variant. Do not use workflow-specific methods as differential oracles after migration.

### Durability and Error Evidence
**Source:** owner session-log fixtures imported in `mod.rs:2323-2327`; `CodingSessionError::PartialCommit` at `coding_session/error.rs:25-28`.
**Apply to:** delegation, profile persistence, export, branch, self-healing, and owner fault tests.

Retain typed operation IDs, pending/committed facts, replay/reopen assertions, event sequence assertions, and structured `PartialCommit` matching. Do not reduce failures to `is_ok()` or string-only checks.

### Deterministic Fixtures
**Source:** owner test imports at `mod.rs:2308-2316` (`FauxProvider`, `FauxResponse`, `FauxToolCall`, `EventStream`, typed messages).
**Apply to:** all streaming workflow tests.

Use offline faux providers, temp sessions, and existing guards; no external provider or network dependency is needed.

### Deletion Proof
**Source:** `product_runtime_boundary_guards.rs` current ledger; procedure `04-RESEARCH.md:209-215`.
**Apply to:** G1 through G4 deletion groups.

Migrate callers, prove receiver-aware zero callers, run focused behavior tests and both boundary guards, run `cargo check -p pi-coding-agent`, delete definitions, then rerun the same checks. Compiler/source failures require caller migration, never a restored or renamed wrapper.

## No Analog Found

No file lacks a usable analog. The only conditional item is `load_plugins(PluginLoadOptions)`: classify each owner call individually. If its custom internal options are genuinely required, retain the narrow private path; otherwise migrate it through `CodingAgentOperation::PluginLoad` and delete the broad method.

## Metadata

**Analog search scope:** `crates/pi-coding-agent/src/coding_session`, `crates/pi-coding-agent/tests`, and public `api`/boundary contracts.
**Files scanned:** 11 directly relevant source/test files plus canonical contract and flow symbols.
**Pattern extraction date:** 2026-07-13
