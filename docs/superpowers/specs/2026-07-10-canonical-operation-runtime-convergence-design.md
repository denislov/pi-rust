# Canonical Operation Runtime Convergence Design

## Status And Scope

This document defines Operation Runtime Stage 9 for `pi-rust`. Stage 8 introduced a stable public facade, but it deliberately stopped with a compatibility-shaped implementation: `CodingAgentSession::run(CodingAgentOperation)` calls deprecated workflow methods, first-party adapters still call those methods directly, and several runtime mutations are not represented in the public operation enum.

Stage 9 makes the existing facade real. It does not add a second facade or a new workflow family. It makes `CodingAgentSession::run(CodingAgentOperation)` the only public dispatcher for live-session product operations, migrates every first-party adapter to it, and deletes the replaced broad workflow methods.

The operation runtime reference architecture remains normative. This design narrows one specific prerequisite for later typed ProductEvent and runtime/client lifecycle work.

## Current Problem

The current owner has two execution semantics:

```text
public run(CodingAgentOperation)
        -> deprecated prompt/compact/invoke/export wrappers
        -> internal Operation dispatcher

first-party adapters
        -> deprecated or crate-private workflow wrappers
        -> internal Operation dispatcher
```

Consequences:

- `run()` is not the canonical dispatcher.
- Every internal `OperationOutcome` addition expands many wrapper-local `unreachable!` matches.
- JSON, print, RPC, and interactive adapters retain narrow `#[allow(deprecated)]` annotations.
- Plugin loading/commands, profile mutation, delegation confirmation, and session navigation have operation metadata internally but no complete public operation contract.
- `summarize_branch_for_navigation()` and `fork_current_session()` preserve adapter-specific execution paths outside the public facade.
- New runtime behavior can still be exposed as another session method instead of an operation variant.

## Goals

1. Make `CodingAgentSession::run(CodingAgentOperation)` directly dispatch internal `Operation` values by their declared `OperationDispatchMode`.
2. Centralize the exhaustive `OperationOutcome` to `CodingAgentOperationOutcome` projection in one place.
3. Cover every currently implemented live-session runtime mutation with a typed public operation contract.
4. Preserve current JSON, print, RPC, and interactive behavior while migrating those adapters to `run()`.
5. Delete replaced broad workflow methods after all first-party callers and tests move to operations.
6. Add source guards that prevent production adapters from reintroducing broad workflow calls or deprecation suppressions.

## Alternatives Considered

### Keep deprecated wrappers as the permanent implementation boundary

This minimizes immediate edits, but it preserves the exact failure mode Stage 9 is meant to remove: `run()` remains a facade over many independent dispatch paths, internal outcome additions keep expanding wrapper matches, and adapters can continue bypassing the public contract. Rejected.

### Add another `CodingAgentRuntime` facade before deleting session methods

A new type could present a smaller API, but it would add a third execution layer while `CodingAgentSession` still owns the real state and compatibility methods. The runtime/client lifecycle is not complete enough to justify the extra owner yet. Defer any rename or wrapper decision to Stage 11 after execution and event convergence. Rejected for Stage 9.

### Merge operation and typed ProductEvent convergence in one stage

This could delete all compatibility layers at once, but it couples adapter execution migration to a large event payload redesign. Failures would be harder to isolate, and RPC/interactive behavior would change on both input and output boundaries simultaneously. Stage 9 therefore converges operation input first; Stage 10 converges product-event output next. Rejected as one combined change.

### Selected approach: direct dispatch, staged caller migration, then deletion

This reuses the internal operation runtime already built in Stages 1-7, changes adapters in reviewable behavior-preserving slices, and lets the compiler plus source guards prove broad paths are gone before Stage 10 begins.

## Non-Goals

- Do not redesign ProductEvent payloads. That is Stage 10.
- Do not remove `CodingAgentEventReceiver`, compatibility `subscribe()`, or `ProductEvent.compatibility_event`. Their deletion depends on typed ProductEvent payload convergence.
- Do not complete reconnect, detach, shutdown, draft, submitted-operation, or public control-handle lifecycle APIs. Those belong to Stage 11.
- Do not add arbitrary Lua Flow nodes, raw session access, provider access, auth access, or session-storage access to plugins.
- Do not implement a new product workflow or start `pi-web-ui`.
- Do not force abort, steer, or follow-up signals into the ordinary operation queue.

## Public Operation Contract

Stage 9 preserves the Stage 8 operation names where they already exist and adds the missing live-session actions:

```rust
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    BranchSummary {
        options: PromptTurnOptions,
        source_leaf_id: String,
        target_leaf_id: String,
        custom_instructions: Option<String>,
        reuse: BranchSummaryReusePolicy,
    },
    SelfHealingEdit(SelfHealingEditRequest),
    InvokeAgent(AgentInvocationOptions),
    InvokeTeam(AgentTeamOptions),
    PluginLoad,
    PluginCommand {
        command_id: String,
        args: serde_json::Value,
    },
    SetDefaultAgentProfile {
        profile_id: ProfileId,
    },
    ApproveDelegation {
        operation_id: String,
        tool_call_id: String,
    },
    RejectDelegation {
        operation_id: String,
        tool_call_id: String,
        reason: String,
    },
    ForkSession {
        target_leaf_id: Option<String>,
    },
    SwitchActiveLeaf {
        target_leaf_id: String,
    },
    ExportCurrent,
    ExportCurrentHtml(PathBuf),
}
```

`BranchSummaryReusePolicy` is explicit because the interactive tree-navigation path currently reuses an existing source/target summary while the direct branch-summary command requests a new summary. That product distinction must survive removal of the adapter-only `summarize_branch_for_navigation()` method.

`PluginLoad` intentionally carries no raw `PluginLoadOptions`. The public operation uses the session-owned project/user discovery roots already derived at session construction. Custom candidate injection remains an internal test concern, not a stable plugin host API.

Static repository-style helpers such as session listing, opening, hydration, and static export are not live-session operation dispatch. They remain outside this enum until Stage 11 defines a complete runtime/session lifecycle contract.

## Public Outcome Contract

The public outcome remains a typed enum, but it covers every operation and exposes a narrow plugin-load projection instead of internal plugin registry types:

```rust
pub enum CodingAgentOperationOutcome {
    Prompt(PromptTurnOutcome),
    Compact(PromptTurnOutcome),
    BranchSummary(PromptTurnOutcome),
    SelfHealingEdit(SelfHealingEditOutcome),
    AgentInvocation(AgentInvocationOutcome),
    AgentTeam(AgentTeamOutcome),
    PluginLoad(CodingAgentPluginLoadOutcome),
    PluginCommand(String),
    DefaultAgentProfileChanged,
    DelegationApproved,
    DelegationRejected,
    SessionForked,
    ActiveLeafSwitched,
    Export(CodingAgentSessionExport),
    ExportHtml(PathBuf),
}
```

`CodingAgentPluginLoadOutcome` contains only loaded plugin ids, diagnostic summaries, and the capability-change flag already required by RPC and interactive adapters. It does not expose `PluginService`, `PluginRegistry`, provider objects, or Lua runtime state.

The Stage 9 outcome is still operation-specific rather than the final envelope with operation id, terminal status, and persistence metadata. That envelope requires the Stage 10 typed event payload and Stage 11 lifecycle correlation work.

## Canonical Dispatch

`CodingAgentSession::run()` performs four steps:

```text
public CodingAgentOperation
        -> crate-private Operation conversion
        -> dispatch selected from OperationMetadata.dispatch_mode
        -> run_operation / run_sync_operation / run_sync_mut_operation
        -> one exhaustive public outcome projection
```

The dispatcher must not call `prompt()`, `compact()`, `invoke_agent()`, or any other workflow facade method. The public-to-internal conversion may use session-owned defaults, such as the default plugin discovery roots, but it must not execute work.

The single internal-to-public outcome projection owns all exhaustive matching. An internal `OperationOutcome` addition should cause one compiler error in the projection, not many expanding wrapper matches.

## Session Navigation Semantics

`SwitchActiveLeaf` mutates the existing persistent session by recording the existing Rust-native `active_leaf.changed` event. It rejects non-persistent sessions and unknown leaves without partial mutation. If the event append succeeds but the manifest update fails, the operation returns explicit `PartialCommit` uncertainty with the admitted operation ID; replay remains authoritative for the durable leaf transition.

`ForkSession` transitions the live `CodingAgentSession` owner to the newly forked Rust-native persistence while retaining owner-scoped runtime and event services. The operation replaces only persistence and replay-derived state such as pending delegation confirmations and startup recovery markers. Plugin registrations, capability generations, workflow/runtime services, product-event retention, replay handles, and existing subscribers remain live. The retained `EventService` publishes the new `SessionOpened` transition, so product-event sequence remains monotonic across the fork. The operation returns `SessionForked`; callers read the new state through `snapshot()` or `view()`.

The existing interactive navigation task already owns and returns the session after navigation, so it can continue to hand the mutated owner back to the interactive loop. Adapters retain their existing product-event receivers across canonical fork and immediately refresh the snapshot. Stage 11 still owns explicit client detach, reconnect, and longer-lived lifecycle semantics.

Fork persistence is cleanup-protected because the current store has no staging/publish primitive. Any failure after the target session directory is created removes that target before returning the original error. If cleanup itself fails, the fork returns `PartialCommit` with the admitted operation ID and the target session identity instead of reporting an ordinary failure.

## Branch Summary Navigation Semantics

The current navigation path performs:

```text
reuse existing source/target summary when available
otherwise run BranchSummary
fork session at target leaf
hydrate the forked session
```

Stage 9 moves the reuse decision into the `BranchSummary` operation contract through `BranchSummaryReusePolicy`. The adapter then runs two explicit operations:

```text
BranchSummary { reuse: ReuseExisting }
ForkSession { target_leaf_id: Some(target) }
```

No adapter calls `BranchSummaryService`, `SessionService`, or an owner-only broad workflow helper.

## Compatibility Deletion

The final Stage 9 tree deletes these live-session workflow methods:

```text
prompt
compact
self_healing_edit
self_healing_edit_with_options
invoke_agent
invoke_team
summarize_branch
summarize_branch_for_navigation
reload_plugins
load_plugins
run_plugin_command
set_default_agent_profile_id
approve_delegation_confirmation
reject_delegation_confirmation
fork_current_session
export_current
export_current_html
```

Internal unit tests that need custom plugin candidates call the crate-private operation dispatcher directly from the owner test module. No production test helper is retained solely to preserve a deleted workflow API.

The compatibility event subscription methods are explicitly not part of this list.

## Adapter Migration

Every first-party adapter constructs `CodingAgentOperation` and matches the one expected `CodingAgentOperationOutcome` variant:

- JSON and print: prompt.
- RPC streaming tasks: prompt, agent invocation, team invocation, delegation approval.
- RPC synchronous commands: self-healing edit, profile change, delegation rejection, plugin load, and plugin command.
- Interactive prompt tasks: prompt, agent invocation, team invocation, delegation approval, compaction, self-healing edit, plugin load, plugin command, branch summary, navigation branch summary, and fork.
- Interactive loop mutations: profile change and delegation rejection.

Adapter-local protocol response types remain unchanged. Stage 9 changes how work is started, not the JSON/RPC/TUI wire or rendering contract.

## Control Boundary

Abort, steer, and follow-up remain high-priority prompt-control signals. They continue to use the owner-issued control channel and are not queued as ordinary operations.

Stage 9 source guards treat that control path as intentional. Stage 11 may expose a stable `CodingAgentControl` or public control handle once operation ids, client connections, detach, shutdown, and reconnect semantics are complete.

## Testing Strategy

Stage 9 uses deterministic offline tests and source guards:

1. Public API tests construct every new operation variant and verify representative outcomes.
2. Owner tests verify async, sync read-only, and sync mutable public operations all route through the declared dispatch mode.
3. Session tests verify profile mutation, delegation confirmation, plugin reload, plugin command, active-leaf switch, and fork behavior through `run()`.
4. Existing JSON, print, RPC, and interactive behavior tests remain the adapter regression suite.
5. Integration tests migrate from broad workflow methods to `run()` without reducing payload, persistence, event, provider-call, or failure coverage.
6. Source guards reject broad workflow method definitions, broad workflow adapter calls, production adapter `#[allow(deprecated)]`, and a `run()` implementation that calls compatibility methods.

## Exit Criteria

Stage 9 is complete only when:

```text
CodingAgentSession::run() directly dispatches internal Operation values
one mapping converts OperationOutcome to CodingAgentOperationOutcome
every implemented live-session runtime mutation has an operation contract
production adapters call run() for product execution
production adapters contain no #[allow(deprecated)] for workflow migration
deleted broad workflow methods no longer compile
branch-summary navigation and fork use operations without adapter-only bypasses
control signals remain an explicit separate priority path
cargo fmt --check passes
cargo test --workspace passes
cargo check --workspace passes
git diff --check passes
```

This exit state is the prerequisite for Stage 10 to remove the compatibility event payload layer without also carrying two execution paths.
