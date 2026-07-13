# Phase 3: Production Adapter Convergence - Research

**Researched:** 2026-07-11
**Domain:** Rust live-session adapter migration to one typed operation dispatcher
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

### Migration Axis And Risk Order
- **D-01:** Organize Phase 3 by adapter risk, not by operation family across multiple adapters.
- **D-02:** The required order is JSON/print first, RPC second, interactive ordinary operations third, and interactive navigation last.
- **D-03:** A lower-risk adapter boundary must complete its behavior-preservation gate before planning or execution advances to the next risk layer.

### JSON And Print Boundary
- **D-04:** JSON, persistent print, and transient print belong in one plan because they share the prompt operation contract.
- **D-05:** The three paths remain separate tasks with separate atomic commits so a regression can be isolated to one adapter/session mode.
- **D-06:** The plan closes with one combined JSON/print parity gate covering output, errors, and session effects.

### RPC Boundary
- **D-07:** Split RPC migration into two plans based on control model rather than one large RPC plan or one plan per operation family.
- **D-08:** The first RPC plan owns background/select-driven operations: prompt, agent, team, delegation approval, and any equivalent work whose correctness depends on `tokio::select!`, control delivery, and product-event forwarding.
- **D-09:** The second RPC plan owns mutation/command operations: self-healing edit, profile mutation, delegation rejection, plugin load, plugin command, and equivalent short-lived typed operations.
- **D-10:** Background control/event behavior must have an independent verification boundary so mutation outcome mapping cannot obscure multiplexing regressions.

### Interactive Boundary
- **D-11:** Split interactive migration into three risk-increasing plans.
- **D-12:** Interactive background plan: prompt, agent, team, manual compaction, self-healing edit, plugin actions, branch summary, and other background operations that retain the existing event/control loop.
- **D-13:** Interactive mutation plan: profile mutation and delegation decisions, preserving existing menus, dialogs, queue state, errors, and visible projections.
- **D-14:** Interactive navigation plan: fork, active-leaf switch, session/owner replacement, subscription continuity, event sequence continuity, snapshot refresh, projection refresh, and visible navigation behavior.
- **D-15:** Navigation is the final migration unit and must be independently accepted after all non-navigation interactive paths are canonical.

### Locked Cross-Phase Constraints
- **D-16:** Production adapters use contracts exported by `pi_coding_agent::api`; internal operations, metadata, plugin options, services, registries, and Flow nodes remain private.
- **D-17:** Session create/open, snapshot/query, subscription, and control remain distinct contracts and are not forced into `CodingAgentOperation`.
- **D-18:** Preserve current adapter semantics rather than using migration as permission to redesign output, wire protocol, TUI projection, event payloads, or control handling.
- **D-19:** Do not delete broad workflow methods in Phase 3. Phase 4 migrates tests and deletes methods only after all production callers are gone.
- **D-20:** Do not create an adapter compatibility facade or shared helper that merely recreates the deleted broad workflow surface under another name.

### the agent's Discretion
- Choose exact plan names and task-level file grouping within the six locked migration boundaries.
- Decide whether small crate-private typed outcome extraction helpers reduce duplication, provided they remain adapter-oriented, do not become a second operation facade, and preserve each adapter's current error/output ownership.
- Select focused test commands and fixture reuse for each task while retaining all existing behavior assertions and deterministic offline execution.
- Determine whether closely related operations within a locked plan require additional atomic commits when source ownership or rollback safety warrants it.

### Deferred Ideas (OUT OF SCOPE)
- Broad owner/public/integration test migration and compatibility method deletion - Phase 4.
- Parser-complete adapter/source boundary guards and final Stage 9 closure audits - Phase 5.
- Typed `ProductEvent` payload convergence and compatibility subscription deletion - Stage 10.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ADAPT-01 | JSON prompts execute through `run(Prompt)` | JSON call-path and outcome projection are mapped below. [VERIFIED: `protocol/json_mode.rs:87-117`] |
| ADAPT-02 | Persistent and transient print prompts execute through the facade | Both print branches and their shared output projection are mapped below. [VERIFIED: `print_mode.rs:108-145`] |
| ADAPT-03 | JSON/print output, errors, and session behavior remain unchanged | Existing JSON, print, session-print, and CLI suites cover these contracts. [VERIFIED: current integration tests] |
| ADAPT-04 | JSON/print contain no broad calls or local deprecation suppression | Current inventory is one JSON call/allow and two print calls/allows; exact closure scans are specified. [VERIFIED: current source audit] |
| RPC-01 | RPC background prompt/agent/team/approval use canonical operations | The four pinned futures and local task outcome envelope are mapped below. [VERIFIED: `protocol/rpc/prompt.rs`] |
| RPC-02 | RPC edit/profile/rejection/plugin work uses canonical operations | Five command families and wire projections are mapped below. [VERIFIED: `protocol/rpc/commands.rs`] |
| RPC-03 | RPC select/control/event/wire/error behavior is preserved | The outer RPC loop, bounded queue, replay cursor, idempotency, and final drains are explicit invariants. [VERIFIED: `protocol/rpc.rs`, `prompt.rs`, `state.rs`] |
| RPC-04 | RPC contains no broad calls or local deprecation suppression | Current inventory is ten broad calls and four local suppressions. [VERIFIED: current source audit] |
| INTER-01 | Interactive background work uses canonical operations | Nine background operation families and existing task result ownership are mapped below. [VERIFIED: `interactive/prompt_task.rs`] |
| INTER-02 | Interactive profile mutation and delegation rejection use operations | Both currently execute in synchronous input processing and require an async integration step. [VERIFIED: `interactive/loop.rs:801-807,1305-1349`] |
| INTER-03 | Interactive fork/navigation use canonical operations and refresh projections | Direct `/fork`, tree fallback fork, and summary-then-fork paths are inventoried. [VERIFIED: `commands.rs`, `loop.rs`, `prompt_task.rs`] |
| INTER-04 | Interactive event/control/subscriber/UI behavior is preserved | The pre-operation receiver, `UiProjection` sequence filter, task result owner return, and hydration order are explicit invariants. [VERIFIED: current source and tests] |
| INTER-05 | Interactive production contains no broad calls or local deprecation suppression | Session calls must be removed without flagging the unrelated `InteractiveRoot::set_default_agent_profile_id` UI method. [VERIFIED: source audit] |
</phase_requirements>

## Summary

Phase 3 is a call-site and outcome-projection migration, not a runtime redesign. Phase 2 already proved the complete 15-variant public facade, metadata-selected dispatch, exhaustive public outcome projection, durable navigation/delegation semantics, event continuity, and privacy boundary. The production adapters currently have zero `CodingAgentOperation` uses and still call broad workflow methods in every adapter family. [VERIFIED: Phase 1 `SCAN-PROD-01/02`, Phase 2 verification, current source audit]

The safe implementation rule is consistent across all six plan boundaries: construct the existing public operation through `crate::api`, await `CodingAgentSession::run`, exhaustively extract the one expected public outcome, and leave the adapter-owned event loop, control loop, response formatting, transcript projection, hydration, and error mapping in place. [VERIFIED: `public_operation.rs:41-180`, adapter source]

The highest planning risk is not the enum mapping. It is async ownership around existing adapters. RPC already pins one operation future inside `tokio::select!`; only that future and its typed result extraction should change. Interactive profile mutation, delegation rejection, and `/fork` currently execute from synchronous input/command handlers even though public `run` is async, so those paths need deliberate integration into the existing async loop/task ownership rather than blocking execution or a new compatibility facade. [VERIFIED: `interactive/loop.rs`, `commands.rs`]

**Primary recommendation:** Execute six plans in the locked order, make each plan replace only its operation future/call and typed result projection, and gate every boundary with existing behavior suites plus a narrowly scoped source audit before advancing.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Operation admission, dispatch, capabilities, durable mutation | Product runtime (`coding_session`) | Session log/services | `CodingAgentSession::run` owns typed execution and remains unchanged in this phase. [VERIFIED: `coding_session/mod.rs:248-261`] |
| JSON/print output and CLI error conversion | JSON/print adapters | Product runtime outcomes | Adapters retain exact serialization/text/error ownership. [VERIFIED: `json_mode.rs`, `print_mode.rs`] |
| RPC wire responses, idempotency, controls, replay, event forwarding | RPC adapter | Product event/control facades | These are protocol semantics and must not move into the runtime or a helper facade. [VERIFIED: `protocol/rpc/`] |
| Interactive menus, transcript, footer, projection, task coordination | Interactive adapter | Product snapshot/event facades | TUI-visible state remains adapter-owned while runtime work moves through `run`. [VERIFIED: `interactive/loop.rs`, `event_bridge.rs`, `root.rs`] |
| Plugin capability execution | Product runtime | RPC/interactive projection | Adapters submit `PluginLoad`/`PluginCommand`; capability-aware execution remains private. [VERIFIED: Phase 2 security and boundary guards] |
| Persistent fork/navigation facts | Product runtime/session log | Interactive hydration | Operations mutate the live owner; the adapter refreshes snapshot/hydration without rebuilding runtime semantics. [VERIFIED: canonical navigation tests] |

## Project Constraints (from AGENTS.md)

- Use CodeGraph before grep/find/file reads when `.codegraph/` exists; this research used it before source scans. [VERIFIED: repository `AGENTS.md` and `.codegraph/`]
- Preserve dependency direction `pi-coding-agent -> pi-agent-core -> pi-ai` and `pi-coding-agent -> pi-tui`; product semantics stay in `pi-coding-agent`. [VERIFIED: project contract]
- New/migrated callers consume `pi_coding_agent::api`; implementation operations, metadata, services, registries, plugin options, and Flow nodes stay private. [VERIFIED: project contract]
- Preserve JSON, print, RPC, interactive, event ordering, control, replay, and navigation behavior. [VERIFIED: project contract]
- Preserve typed durable facts, operation IDs, append/manifest ordering, recovery markers, and explicit `PartialCommit`. [VERIFIED: project contract and Phase 2 verification]
- Keep deterministic offline fixtures and existing behavior assertions; do not replace them with compile-only checks. [VERIFIED: project contract]
- Migrate production before tests/method deletion; Phase 3 must not delete compatibility methods. [VERIFIED: project contract and D-19]
- Use standard `rustfmt`, typed errors, narrow lint suppressions, and curated `api` exports. [VERIFIED: repository conventions]
- Final milestone verification includes format, focused crate tests, workspace test/check, source audits, and `git diff --check`; Phase 3 should at least run focused crate gates and leave full closure to Phase 5. [VERIFIED: project contract]

## Standard Stack

### Core

| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| Rust | 1.96.0 installed; edition 2024 | Adapter and runtime implementation | Existing workspace language/toolchain; no change required. [VERIFIED: `rustc --version`, manifests] |
| Tokio | 1.52.3 | Async tasks, channels, `tokio::select!`, controls | Existing RPC/interactive concurrency substrate. [VERIFIED: `Cargo.lock`, source] |
| Serde / serde_json | 1.0.228 / 1.0.150 | RPC and JSON wire contracts | Existing typed serialization; wire shape must remain unchanged. [VERIFIED: `Cargo.lock`, source] |
| Rust test harness + Tokio tests | toolchain | Deterministic unit/integration coverage | Existing project test architecture. [VERIFIED: test tree] |

### Supporting

| Component | Version | Purpose | When to Use |
|-----------|---------|---------|-------------|
| `pi_coding_agent::api` | workspace 0.1.0 | Stable operation, outcome, support, lifecycle, event, and control contracts | All migrated production adapter inputs/outcomes. [VERIFIED: `src/lib.rs:60-117`] |
| CodeGraph CLI | 1.2.0 | Symbol/call-path discovery | Before source searches during planning/implementation. [VERIFIED: local environment] |
| Faux providers/tempfile/test harnesses | workspace fixtures | Offline behavior verification | Every adapter parity gate. [VERIFIED: existing integration tests] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Direct exhaustive outcome match in each adapter boundary | One shared extraction helper for all operations | Rejected by default: it can become a second facade and obscure adapter-specific error ownership. A tiny adapter-local typed extractor is acceptable only when repeated in the same adapter. [VERIFIED: D-20] |
| Preserve current select/task topology | Rewrite RPC/interactive task orchestration | Rejected: combines input convergence with concurrency/protocol redesign and expands regression surface. [VERIFIED: D-18 and design spec] |
| Async integration for interactive mutations | Blocking on `run` in synchronous handlers | Rejected: risks runtime blocking/deadlock and changes UI timing. [VERIFIED: current async architecture] |

**Installation:** None. No new crate or external package is required. [VERIFIED: current facade and stack already implement all required behavior]

## Package Legitimacy Audit

Not applicable. Phase 3 installs no external packages and changes no dependency manifests. [VERIFIED: scope and current implementation]

## Architecture Patterns

### System Architecture Diagram

```text
JSON / print input          RPC JSONL command               TUI action
       |                           |                            |
       v                           v                            v
adapter builds public CodingAgentOperation through crate::api
       |                           |                            |
       +---------------------------+----------------------------+
                                   |
                                   v
                    CodingAgentSession::run(operation)
                                   |
                    typed admission + metadata dispatch
                                   |
                   private Flow/services/session transaction
                                   |
                  CodingAgentOperationOutcome + ProductEvent
                                   |
       +---------------------------+----------------------------+
       |                           |                            |
JSON/print projection       RPC response/event queue      TUI projection/hydration
```

### Recommended Project Structure

```text
crates/pi-coding-agent/src/
├── protocol/json_mode.rs        # JSON Prompt operation + event serialization
├── print_mode.rs                # persistent/transient Prompt operation + text/error mapping
├── protocol/rpc/
│   ├── prompt.rs                # pinned background operation futures and event forwarding
│   ├── commands.rs              # short-lived mutation operations and wire projection
│   └── state.rs                 # adapter-local task result and idempotency state
└── interactive/
    ├── prompt_task.rs           # background operations, controls, events, navigation fork
    ├── loop.rs                  # async mutation integration and task completion
    ├── commands.rs              # UI intent only; avoid direct runtime mutation
    ├── session_actions.rs       # lifecycle/query helpers, not operation bypasses
    └── event_bridge.rs          # unchanged sequence-aware projection
```

### Pattern 1: Expected Public Outcome Extraction

Use the public facade type and match exactly one expected result at the adapter boundary. Preserve the existing adapter error type after extraction. [VERIFIED: Phase 2 public operation contract]

```rust
use crate::api::{CodingAgentOperation, CodingAgentOperationOutcome};

let outcome = session
    .run(CodingAgentOperation::Prompt(prompt_options))
    .await?;
let prompt_outcome = match outcome {
    CodingAgentOperationOutcome::Prompt(outcome) => outcome,
    _ => unreachable!("prompt operation returned a non-prompt public outcome"),
};
```

### Pattern 2: Replace the Pinned Future, Not the Select Loop

RPC and interactive background tasks should keep receiver/control branches, bounded queues, replay cursors, final drains, task result ownership, and response ordering. Only replace the pinned workflow future with `session.run(...)` and map its public result into the existing adapter-local task result. [VERIFIED: current `tokio::select!` paths; Tokio 1.52.3 official crate documentation]

### Pattern 3: Navigation as Two Explicit Operations

Tree navigation that summarizes an abandoned branch must run `BranchSummary { reuse: ReuseExisting }`, continue using the pre-fork product-event receiver, then run `ForkSession { target_leaf_id: Some(target) }` on the same owner, send/refresh the snapshot, hydrate, and return the mutated owner. [VERIFIED: design spec and Phase 2 navigation behavior]

Direct `/branch-summary` uses `AlwaysCreate`; it must not inherit navigation reuse semantics. [VERIFIED: public contract and existing two-path behavior]

### Pattern 4: Async Interactive Mutation Boundary

`SetDefaultAgentProfile`, `RejectDelegation`, and live `/fork` cannot remain direct calls inside synchronous input/command handlers because `run` is async. Integrate them into the existing async loop/task lifecycle while preserving when local UI state changes, how errors are surfaced, how events are drained, and which session owner is returned. [VERIFIED: current function signatures and call paths]

### Anti-Patterns to Avoid

- **Second facade:** Do not add `run_prompt`, `run_profile_change`, or a generic adapter runtime wrapper that mirrors old methods. [VERIFIED: D-20]
- **Internal imports:** Do not import private `Operation`, dispatch metadata, services, plugin load options, or Flow types into adapters. [VERIFIED: D-16]
- **Select-loop rewrite:** Do not combine operation migration with channel/control/replay refactors. [VERIFIED: D-18]
- **Outcome erasure:** Do not discard `CodingAgentOperationOutcome` without checking the expected variant. [VERIFIED: public contract]
- **UI method false positive:** Do not rename or reject `InteractiveRoot::set_default_agent_profile_id`; it is local presentation state, not the session workflow method. [VERIFIED: historical plan and current source]
- **Early method deletion:** Do not delete broad workflow methods or update all test callers in Phase 3. [VERIFIED: D-19]

## Production Call-Path Audit

### JSON And Print

| Path | Current Call | Canonical Operation | Required Projection | Preserve |
|------|--------------|---------------------|---------------------|----------|
| JSON background prompt | `session.prompt` | `Prompt` | `Outcome::Prompt` into existing oneshot result | Header/AgentStart ordering, product-event select/drain, exit code/stderr, synthetic failure event. [VERIFIED: `json_mode.rs`] |
| Persistent print | `session.prompt` | `Prompt` | `Outcome::Prompt` into `print_text_from_prompt_outcome` | Session target/open/fork behavior and `CliError` mapping. [VERIFIED: `print_mode.rs`] |
| Transient print | `session.prompt` | `Prompt` | same | Non-persistent target rejection and no session files. [VERIFIED: `print_mode.rs`] |

Current closure inventory: JSON has one broad prompt call and one local deprecation allow; print has two prompt calls and two local allows. [VERIFIED: `rg` source audit]

### RPC Background / Select-Driven

| Current Future | Canonical Operation | Existing Adapter Result | Critical Invariants |
|----------------|---------------------|-------------------------|---------------------|
| prompt | `Prompt` | `CodingOperationOutcome::Prompt(Result<PromptTurnOutcome, CliError>)` | Prompt control handle remains separate; response and `AgentStart` precede task; select branches and final drain unchanged. [VERIFIED: `prompt.rs:825-1074`] |
| agent | `InvokeAgent` | `AgentInvocation(...)` | Existing profile validation, busy/idempotency responses, event queue, task completion ownership. [VERIFIED: `prompt.rs:210-430`] |
| team | `InvokeTeam` | `AgentTeam(...)` | Existing team validation, no control handle, abort semantics, event forwarding. [VERIFIED: `prompt.rs:432-647`] |
| delegation approval | `ApproveDelegation` | `DelegationApproval(Result<(), CliError>)` | Dynamic operation kind from pending target, response shape, queue/replay behavior. [VERIFIED: `prompt.rs:649-823`] |

The outer RPC loop selects among input, queued product events, and task completion; `RpcRunningPrompt` retains bounded event receiver, optional control, replay handle, applied/replayed sequence cursors, and idempotency key. Those fields and their update order are adapter protocol state, not migration scaffolding. [VERIFIED: `rpc.rs:20-115`, `state.rs:31-85`]

### RPC Mutation / Command

| Command | Current Call | Canonical Operation | Preserve |
|---------|--------------|---------------------|----------|
| self-healing edit | `self_healing_edit_with_options` | `SelfHealingEdit` | Structured success/error data, check output, repair attempts, event drain, session restoration. [VERIFIED: `commands.rs:501-646`] |
| set default profile | `set_default_agent_profile_id` | `SetDefaultAgentProfile` | Pre-validation, idempotency, response JSON, event drain. [VERIFIED: `commands.rs:800-879`] |
| reject delegation | `reject_delegation_confirmation` | `RejectDelegation` | Pending lookup, default reason, response payload, event drain. [VERIFIED: `commands.rs:922-1067`] |
| reload | `reload_plugins` | `PluginLoad` | Same RPC fields via `CodingAgentPluginLoadOutcome`; session always returned. [VERIFIED: `commands.rs:1162-1203`] |
| plugin command | optional `reload_plugins`, then `run_plugin_command` | `PluginLoad`, then `PluginCommand` | Load-before-first-command behavior, commandId/output shape, error protocol. [VERIFIED: `commands.rs:1090-1159`] |

Current closure inventory: RPC prompt has four broad calls and three local allows; RPC commands has six broad calls and one local allow. [VERIFIED: `rg` source audit]

### Interactive Background

| Task | Canonical Operation | Existing Result Ownership |
|------|---------------------|---------------------------|
| prompt | `Prompt` | Keep `CodingPromptTaskResult` and prompt controls. [VERIFIED: `prompt_task.rs:610-679`] |
| agent | `InvokeAgent` | Keep `AgentInvocationTaskResult`; typed outcome may be validated then discarded as today. [VERIFIED: `prompt_task.rs:682-752`] |
| team | `InvokeTeam` | Keep `AgentTeamOutcome` used by finish logic. [VERIFIED: `prompt_task.rs:755-809`] |
| delegation approval | `ApproveDelegation` | Keep session return and event drain. [VERIFIED: `prompt_task.rs:811-848`] |
| compact | `Compact` | Keep prompt-like result and current unsupported abort behavior. [VERIFIED: `prompt_task.rs:851-904`] |
| self-healing | `SelfHealingEdit` | Keep structured diagnostic projection. [VERIFIED: `prompt_task.rs:907-955`] |
| plugin reload | `PluginLoad` | Change task outcome type to public `CodingAgentPluginLoadOutcome`; preserve UI extension refresh. [VERIFIED: `prompt_task.rs:957-1015`] |
| plugin command | optional `PluginLoad`, then `PluginCommand` | Preserve plugin extension refresh and visible message. [VERIFIED: `prompt_task.rs:1017-1088`] |
| direct branch summary | `BranchSummary/AlwaysCreate` | Keep prompt-like outcome and no hydration. [VERIFIED: `prompt_task.rs:1159-1221`] |

Current closure inventory: `prompt_task.rs` has eleven broad workflow calls and six local deprecation allows. [VERIFIED: `rg` source audit]

### Interactive Mutations And Navigation

- Default profile mutation currently calls the session method from synchronous input processing after local selection state is consumed. The canonical async call must preserve local menu text and `prompt_context.default_agent_profile_id` behavior, including error propagation. [VERIFIED: `loop.rs:801-807`, `root.rs:1290-1304`]
- Delegation rejection currently subscribes, performs a synchronous mutation, drains product events through `CodingEventBridge`, and emits a fallback visible notice only when no events arrive. Make this path async without changing those semantics. [VERIFIED: `loop.rs:1305-1349`]
- Direct `/fork` currently runs synchronously from `commands.rs` through static `fork_rust_native_choice`; tree navigation without an in-memory owner uses the same helper; summary navigation uses private `summarize_branch_for_navigation` then private `fork_current_session`. All live fork/navigation work must converge on public operations while clone/open/query helpers remain lifecycle contracts. [VERIFIED: `commands.rs:812-843`, `loop.rs:872-941`, `prompt_task.rs:1223-1282`]
- There is no current first-party production call to `CodingAgentOperation::SwitchActiveLeaf`. Do not invent a new visible navigation behavior solely to create one; retain a closure scan and use `SwitchActiveLeaf` only if a current adapter path is identified during implementation. Existing tree navigation is contractually summary-then-fork and tests assert `session.forked`. [VERIFIED: current production scan, design spec, interactive tests]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Operation admission/dispatch | Adapter-local dispatch tables | `CodingAgentSession::run` | Metadata, capabilities, exclusivity, persistence, and error semantics already live there. [VERIFIED: Phase 2] |
| Product event replay/dedup | New sequence cache | Existing `ProductEventReplayHandle`, adapter cursors, `UiProjection` | Current tests cover gaps, lag, replay overlap, and stale events. [VERIFIED: RPC/UI tests] |
| Prompt lifecycle control | Put abort/steer/follow-up into operations | Existing `PromptControlHandle` | D-17 keeps control separate and current select loops depend on it. [VERIFIED: context and source] |
| Plugin projection | Expose plugin services/registries | Public plugin load outcome plus existing query methods | Avoids capability/private-state leakage. [VERIFIED: Phase 2 security] |
| Session fork persistence | Adapter filesystem manipulation | `ForkSession` operation | Preserves operation ID, replay, owner runtime, events, and `PartialCommit`. [VERIFIED: canonical navigation tests] |
| Rust source parsing | Expand the handwritten guard parser now | Narrow Phase 3 checks; Phase 5 parser hardening | Parser-complete guard work is explicitly deferred. [VERIFIED: context and Phase 2 review] |

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | Rust-native `session.json` and `events.jsonl` store semantic operation/session facts, not broad Rust method names. Existing event names such as `self_healing_edit.*` are product data and must remain unchanged. [VERIFIED: session-log source scan] | No data migration. Verify adapter migration preserves emitted facts, operation IDs, append/manifest ordering, and replay results. |
| Live service config | None. This is a local native CLI/TUI refactor; no external control-plane configuration stores adapter method names. [VERIFIED: architecture/project scope] | None. |
| OS-registered state | None identified. No systemd/launchd/scheduler registration is part of the repository runtime contract. [VERIFIED: repository architecture and source inventory] | None. |
| Secrets/env vars | Existing provider credentials plus `PI_RUST_DIR`/`PI_SESSION_DIR` are unaffected; no environment key is renamed. [VERIFIED: configuration and adapter scans] | No secret migration. Retain existing auth resolution and avoid printing credentials. |
| Build artifacts / installed packages | `target/` binaries may contain the pre-migration implementation until rebuilt; no package name or install registration changes. [VERIFIED: Cargo workspace] | Rebuild through focused tests/checks; no artifact migration or reinstall step. |

## Common Pitfalls

### Pitfall 1: Correct Operation, Wrong Adapter Outcome
**What goes wrong:** The call reaches `run`, but the adapter changes error/output shape or silently accepts an unexpected outcome. [VERIFIED: typed public outcome contract]
**How to avoid:** Exhaustively match the expected variant at the adapter boundary and feed the extracted value into the existing projection function.
**Warning signs:** New generic error strings, changed RPC JSON fields, or duplicated output conversion.

### Pitfall 2: Dropped Final Events
**What goes wrong:** Operation completion wins `select!` before queued product events are forwarded. [VERIFIED: existing final-drain code]
**How to avoid:** Preserve each post-completion `try_recv`/async drain exactly and keep the receiver alive across the operation.
**Warning signs:** Missing terminal event, RPC response before final event where tests expect the reverse, or stale UI status.

### Pitfall 3: Breaking Control Multiplexing
**What goes wrong:** Reconstructing or moving the operation future changes abort/steer/follow-up delivery or cancellation behavior. [CITED: Tokio 1.52.3 installed crate docs]
**How to avoid:** Pin `session.run(operation)` in the same scope and retain every current `tokio::select!` branch and guard.
**Warning signs:** busy state never clears, Ctrl-C stops working, or steer/follow-up becomes a new prompt.

### Pitfall 4: Losing Session Owner Or Subscriber Continuity
**What goes wrong:** Fork/navigation creates a replacement owner or receiver outside the canonical mutation, dropping event sequence continuity or plugin/runtime state. [VERIFIED: Phase 2 navigation tests]
**How to avoid:** Run `ForkSession` on the same owner, retain the pre-fork receiver, then refresh snapshot/hydration and return that owner.
**Warning signs:** sequence resets, `SessionOpened` missing, footer/transcript stale, or post-fork plugin/profile state disappears.

### Pitfall 5: Async Migration Hidden in a Sync Handler
**What goes wrong:** Interactive profile/rejection/fork uses blocking execution or is fire-and-forget, changing error/UI timing. [VERIFIED: current signatures]
**How to avoid:** Route work into the existing async loop/task completion boundary and make the smallest necessary handler async transition.
**Warning signs:** `block_on`, detached task without session return, UI updated even though runtime mutation failed.

### Pitfall 6: Over-Broad Source Guard
**What goes wrong:** A text scan flags `InteractiveRoot::set_default_agent_profile_id`, which is legitimate local UI state, or expands into Phase 5 parser work. [VERIFIED: current source and deferred scope]
**How to avoid:** Scope Phase 3 checks to session receiver calls/deprecation attributes and leave recursive parser hardening to Phase 5.

## Code Examples

### RPC Pinned Future

```rust
let mut invocation = Box::pin(session.run(CodingAgentOperation::InvokeAgent(
    invocation_options,
)));

// Keep the existing tokio::select! branches unchanged.
let outcome = match invocation.await.map_err(CliError::from)? {
    CodingAgentOperationOutcome::AgentInvocation(outcome) => outcome,
    _ => unreachable!("agent operation returned a different public outcome"),
};
```

Source basis: current RPC pinned-future topology plus the Phase 2 public facade. [VERIFIED: `protocol/rpc/prompt.rs`, `public_operation.rs`]

### Navigation Sequence

```rust
let summary = session
    .run(CodingAgentOperation::BranchSummary {
        options: branch_options,
        source_leaf_id,
        target_leaf_id: target_leaf_id.clone(),
        custom_instructions: None,
        reuse: BranchSummaryReusePolicy::ReuseExisting,
    })
    .await?;

let fork = session
    .run(CodingAgentOperation::ForkSession {
        target_leaf_id: Some(target_leaf_id),
    })
    .await?;
```

Source basis: Stage 9 design and Phase 2 durability evidence. [VERIFIED: design spec, canonical navigation tests]

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Adapters call workflow-specific session methods | Adapters submit public typed operations to one dispatcher | Stage 9 Phase 3 target | One admitted path without changing external adapter contracts. [VERIFIED: milestone requirements] |
| Navigation uses adapter-only summary/fork helpers | Explicit `BranchSummary/ReuseExisting` then `ForkSession` | Public contract completed in Phase 2 | Reuse and durable fork semantics stay explicit and testable. [VERIFIED: Phase 2] |
| Plugin adapters consume internal `PluginLoadOutcome` | Public `CodingAgentPluginLoadOutcome` | Phase 2 facade | Keeps capability internals private while retaining adapter data. [VERIFIED: `public_operation.rs`] |

**Deprecated/outdated:** Broad live-session workflow methods remain temporarily for Phase 4 test migration and deletion. Production adapter use is outdated after this phase, but definitions must remain until Phase 4. [VERIFIED: D-19]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | None. Planning recommendations are grounded in current source, tests, planning contracts, Git history, or installed official crate documentation. | — | — |

## Open Questions (RESOLVED)

1. **RESOLVED: Phase 3 uses narrow executable adapter-specific source guards/audits.**
   - What we know: requirements demand zero production calls/suppressions, while parser-complete recursive hardening is Phase 5. [VERIFIED: requirements/context]
   - Decision: Plans 03-01, 03-03, and 03-06 add narrowly scoped checks in the existing boundary suite where they provide isolated adapter gates. Recursive parser-complete source-guard hardening remains Phase 5; Phase 3 does not refactor or generalize the scanner/parser.

2. **RESOLVED: Do not introduce `SwitchActiveLeaf` in Phase 3.**
   - What we know: current production scans show none; existing tree navigation is summary-then-fork and behavior tests assert a fork. [VERIFIED: source/tests]
   - Decision: Current CodeGraph/research found no first-party production adapter caller. Plan 03-06 preserves summary-then-fork navigation and keeps only an implementation-time closure scan for a genuinely missed caller; it does not invent new switch behavior.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust compiler | build/test | yes | 1.96.0 | none needed |
| Cargo | build/test | yes | 1.96.0 | none needed |
| rustfmt | format gate | yes | 1.9.0-stable | none needed |
| CodeGraph CLI | repository navigation | yes | 1.2.0 | `rg` only after CodeGraph per AGENTS.md |

No missing dependencies. No external service or network access is required for implementation or focused tests. [VERIFIED: environment and deterministic fixtures]

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in harness + Tokio 1.52.3 async tests |
| Config file | Cargo manifests; no separate test config |
| Quick run command | `cargo test -p pi-coding-agent --test <adapter-suite> <test-name> -- --exact` |
| Full phase suite | `cargo test -p pi-coding-agent` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADAPT-01, ADAPT-03 | JSON event/output/error/session parity | integration | `cargo test -p pi-coding-agent --test json_mode -- --nocapture` | yes |
| ADAPT-02, ADAPT-03 | Print persistent/transient/output/error/session parity | integration | `cargo test -p pi-coding-agent --test print_mode -- --nocapture` and `cargo test -p pi-coding-agent --test session_print_mode -- --nocapture` | yes |
| ADAPT-04 | No JSON/print broad calls or local allows | source guard/audit | focused existing/new boundary check plus exact `rg` audit | partial; narrow guard may be Wave 0 |
| RPC-01, RPC-03 | Prompt events/control/idempotency | integration | `cargo test -p pi-coding-agent --test rpc_mode rpc_prompt_returns_response_then_agent_events -- --exact`; also abort/steer/follow-up named tests | yes |
| RPC-01, RPC-03 | Agent/team/approval response-before-events and busy behavior | integration | `cargo test -p pi-coding-agent --test rpc_mode rpc_invoke_agent_returns_response_then_agent_events -- --exact`; equivalent team/approval tests | yes |
| RPC-02 | Self-healing/profile/rejection/plugin command projections | integration | `cargo test -p pi-coding-agent --test rpc_mode -- --nocapture` | yes |
| RPC-03 | Persistent and disabled-session prompt state | integration | `cargo test -p pi-coding-agent --test protocol_sessions -- --nocapture` | yes |
| RPC-04 | No RPC broad calls or local allows | source guard/audit | focused existing/new boundary check plus exact `rg` audit | partial; narrow guard may be Wave 0 |
| INTER-01, INTER-04 | Prompt control, agent/team, self-heal, compact | integration | named tests in `interactive_mode.rs` and `interactive_abort.rs` | yes |
| INTER-01 | Plugin reload/command/dialog projections | integration | `cargo test -p pi-coding-agent --test interactive_sessions interactive_plugin_command_runs_loaded_lua_plugin_command -- --exact` plus reload test | yes |
| INTER-01 | Direct branch-summary behavior | integration | add/confirm a focused `/branch-summary` test preserving summary visibility/persistence | no clear dedicated test; Wave 0 |
| INTER-02, INTER-04 | Profile mutation menu/state/session behavior | integration | add focused interactive profile selection/persistence test | no clear end-to-end test; Wave 0 |
| INTER-02, INTER-04 | Delegation rejection event/fallback-visible behavior | integration/unit | retain `interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary`; add behavior test if handler becomes async | partial; likely Wave 0 update |
| INTER-03, INTER-04 | `/fork` and tree summary/fork/hydration | integration | `cargo test -p pi-coding-agent --test interactive_mode scripted_interactive_fork_after_rust_native_prompt_creates_session -- --exact`; two navigation tests in `interactive_sessions.rs` | yes |
| INTER-05 | No interactive session broad calls or local allows | source guard/audit | focused existing/new boundary check that excludes local UI methods, plus exact `rg` audit | partial; narrow guard may be Wave 0 |

### Sampling Rate

- **Per task commit:** Run the exact named tests for the adapter path plus its scoped source audit.
- **Per plan boundary:** Run the complete relevant integration suite(s) and `cargo check -p pi-coding-agent`.
- **Phase gate:** `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, scoped source audits, and `git diff --check`; workspace-wide closure remains mandatory by milestone completion. [VERIFIED: project contract]

### Wave 0 Gaps

- [ ] Narrow JSON/print, RPC, and interactive canonical-call/deprecation checks if the planner chooses executable RED/GREEN guards rather than command-only audits.
- [ ] Focused direct interactive `/branch-summary` parity test.
- [ ] Focused interactive default-profile mutation behavior/persistence test.
- [ ] Adapt or add delegation-rejection behavior coverage when converting the synchronous handler to async.

Existing infrastructure, faux providers, tempfile sessions, and harness helpers are sufficient; no framework or fixture package is missing. [VERIFIED: test tree]

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes, unchanged | Preserve existing API-key/auth diagnostic resolution; operations receive existing prompt options. [VERIFIED: adapter source] |
| V3 Session Management | yes | Use canonical persistent owner operations, replay-authoritative state, monotonic product-event sequences, and snapshot recovery. [VERIFIED: Phase 2] |
| V4 Access Control | yes | Route all runtime-affecting work through admission and immutable capability snapshots; do not expose services. [VERIFIED: runtime boundary guards] |
| V5 Input Validation | yes | Keep serde RPC parsing, `ProfileId` validation, pending-delegation lookup, command parsing, and typed operation construction. [VERIFIED: adapter source] |
| V6 Cryptography | no new control | Do not change credential handling or implement cryptography in this phase. [VERIFIED: scope] |

### Known Threat Patterns For This Refactor

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Adapter bypasses admission via broad method/private service | Elevation of Privilege / Tampering | Public operation facade only; scoped source checks. [VERIFIED: Phase 2 security] |
| RPC/TUI drops or reorders terminal events | Repudiation / Denial of Service | Preserve receiver, select branches, sequence cursors, final drain, and snapshot recovery. [VERIFIED: source/tests] |
| Fork/navigation loses owner runtime or subscriber continuity | Tampering / Denial of Service | Mutate same owner through `ForkSession`, retain receiver, refresh snapshot/hydration. [VERIFIED: canonical durability tests] |
| Plugin path bypasses capability-aware execution | Elevation of Privilege | Submit `PluginLoad`/`PluginCommand`; keep service private. [VERIFIED: boundary guards] |
| Unexpected public outcome is ignored | Tampering / Repudiation | Exhaustive expected-variant match at each adapter boundary. [VERIFIED: typed contract] |
| Async operation is blocked/detached from sync UI handler | Denial of Service | Integrate with existing async loop/task owner and return the session owner. [VERIFIED: current architecture] |
| Credentials leak through new diagnostics | Information Disclosure | Preserve existing error/output projection and never log API keys. [VERIFIED: existing auth boundary] |

## Sources

### Primary (HIGH confidence)

- Current CodeGraph call paths and on-disk source under `crates/pi-coding-agent/src/`.
- `.planning/phases/01-evidence-based-baseline/01-AUDIT.md`.
- `.planning/phases/02-canonical-facade-correctness/02-VERIFICATION.md` and `02-SECURITY.md`.
- Current deterministic integration/unit tests under `crates/pi-coding-agent/tests/` and owner modules.
- `Cargo.lock` and installed Tokio 1.52.3 crate source documentation.

### Secondary (MEDIUM confidence)

- `docs/superpowers/specs/2026-07-10-canonical-operation-runtime-convergence-design.md`.
- `docs/superpowers/plans/2026-07-10-canonical-operation-runtime-convergence-plan.md` as historical implementation evidence only.

### Tertiary (LOW confidence)

- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions and APIs are present in the current lockfile, toolchain, and source.
- Architecture: HIGH - production call paths were traced with CodeGraph and direct source inspection, then reconciled with Phase 1/2 evidence.
- Pitfalls: HIGH - each risk maps to a current select/task/session/projection path and existing behavior test.
- External documentation: MEDIUM - the research seam selected Context7, which was unavailable; the same claim was checked against the installed official Tokio 1.52.3 crate documentation and cached through the research seam.

**Research date:** 2026-07-11
**Valid until:** 2026-08-10, or until adapter/runtime source changes materially
