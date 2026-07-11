# Phase 3: Production Adapter Convergence - Pattern Map

**Mapped:** 2026-07-12
**Files classified:** 17 production/test files
**Analog families:** 5
**Scope:** Phase 3 adapter call-site convergence only; no compatibility-method deletion, event redesign, or Phase 5 parser hardening

## Mapping Rule

Phase 3 does not need a new runtime pattern. Every target already has the correct adapter-owned shell. The implementation pattern is:

1. Import `CodingAgentOperation` and `CodingAgentOperationOutcome` through `crate::api`.
2. Replace only the broad workflow call/future with `session.run(operation)`.
3. Exhaustively extract the expected public outcome at the adapter boundary.
4. Feed the extracted value into the existing output, wire, event, task-result, hydration, and error projection.
5. Preserve lifecycle/query/subscription/control calls because they are intentionally outside `CodingAgentOperation`.

Do not introduce a shared compatibility facade. Small extraction helpers are acceptable only when private to one adapter and when they preserve that adapter's existing error ownership.

## File Classification

| New/Modified File | Role | Data Flow | Closest Current Analog | Match Quality |
|---|---|---|---|---|
| `crates/pi-coding-agent/src/protocol/json_mode.rs` | protocol adapter/controller | streaming + request-response | Its existing `run_json_prompt` select/drain shell; canonical input/output types in `coding_session/public_operation.rs` | exact shell + exact contract |
| `crates/pi-coding-agent/src/print_mode.rs` | CLI adapter/controller | request-response + file-backed session lifecycle | Its existing persistent/transient split and `print_text_from_prompt_outcome` | exact shell |
| `crates/pi-coding-agent/src/protocol/rpc/prompt.rs` | protocol background-operation controller | streaming + pub-sub + control multiplexing | Existing pinned workflow futures inside unchanged `tokio::select!` loops | exact shell |
| `crates/pi-coding-agent/src/protocol/rpc/state.rs` | adapter-local task state/model | event-driven + pub-sub | Existing `CodingRunningPrompt` and `CodingOperationTaskResult` owner envelope | exact shell |
| `crates/pi-coding-agent/src/protocol/rpc/commands.rs` | protocol mutation controller | request-response + event drain | Existing self-heal/profile/delegation/plugin response projection | exact shell |
| `crates/pi-coding-agent/src/interactive/prompt_task.rs` | background task owner | streaming + control multiplexing + owner transfer | Existing `PromptTaskResult` envelopes and select/drain loops | exact shell |
| `crates/pi-coding-agent/src/interactive/loop.rs` | TUI controller/coordinator | event-driven + async task completion + projection | Existing `coding_session.take()` -> task -> `finish_prompt` owner restoration | exact shell |
| `crates/pi-coding-agent/src/interactive/commands.rs` | UI intent router | event-driven | Existing action/pending-request pattern used by compact, summary, and plugin commands | exact role match |
| `crates/pi-coding-agent/src/interactive/session_actions.rs` | lifecycle/query utility | file I/O + transform | Keep clone/tree/hydration helpers; live fork moves to owner operation paths | partial, scope fence |
| `crates/pi-coding-agent/src/interactive/event_bridge.rs` | event projection adapter | transform + event-driven | Existing `ProductEvent` -> compatibility event -> `UiEvent` bridge | exact preserved projection |
| `crates/pi-coding-agent/tests/json_mode.rs` | integration test | serialized output + persistent effects | Existing lifecycle/error/session assertions | exact verification analog |
| `crates/pi-coding-agent/tests/print_mode.rs` | integration test | output/error + transient effects | Existing output and no-session-file assertions | exact verification analog |
| `crates/pi-coding-agent/tests/session_print_mode.rs` | integration test | file I/O + replay/persistence | Existing `session.json`/`events.jsonl` assertions | exact verification analog |
| `crates/pi-coding-agent/tests/rpc_mode.rs` | integration test | JSONL streaming + controls + mutations | Existing response-before-event, live-event, abort/steer/follow-up, mutation tests | exact verification analog |
| `crates/pi-coding-agent/tests/protocol_sessions.rs` | integration test | persistent/transient session behavior | Existing RPC session-state coverage | exact verification analog |
| `crates/pi-coding-agent/tests/interactive_mode.rs`, `interactive_abort.rs`, `interactive_sessions.rs` | integration tests | scripted TUI + controls + navigation | Existing scripted harness assertions | exact verification analog |
| `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs` | architecture/source guard | batch source scan | Existing adapter scans and bounded-RPC-queue guard | exact guard framework |

`interactive/root.rs` is a projection dependency, not a required operation-call migration target. In particular, `InteractiveRoot::set_default_agent_profile_id` is legitimate local UI state and must not be flagged as the deprecated session method.

## Pattern Assignments

### Family 1: Public Operation Construction And Exhaustive Outcome Extraction

**Apply to:** every production adapter file.

**Canonical contract:** `crates/pi-coding-agent/src/coding_session/public_operation.rs:42-104`

```rust
pub enum CodingAgentOperation {
    Prompt(PromptTurnOptions),
    Compact(PromptTurnOptions),
    // ...
    PluginLoad,
    PluginCommand { command_id: String, args: serde_json::Value },
    SetDefaultAgentProfile { profile_id: ProfileId },
    ApproveDelegation { operation_id: String, tool_call_id: String },
    RejectDelegation { operation_id: String, tool_call_id: String, reason: String },
    ForkSession { target_leaf_id: Option<String> },
    // ...
}

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
    // ...
}
```

**Exhaustiveness analog:** `crates/pi-coding-agent/src/coding_session/public_operation.rs:161-180`

```rust
match outcome {
    OperationOutcome::Prompt(outcome) => Self::Prompt(outcome),
    OperationOutcome::ManualCompaction(outcome) => Self::Compact(outcome),
    OperationOutcome::PluginLoad(outcome) => Self::PluginLoad(outcome.into()),
    OperationOutcome::PluginCommand(output) => Self::PluginCommand(output),
    OperationOutcome::DelegationApproval => Self::DelegationApproved,
    OperationOutcome::DelegationRejection => Self::DelegationRejected,
    OperationOutcome::BranchSummary(outcome) => Self::BranchSummary(outcome),
    OperationOutcome::SelfHealingEdit(outcome) => Self::SelfHealingEdit(outcome),
    OperationOutcome::AgentInvocation(outcome) => Self::AgentInvocation(outcome),
    OperationOutcome::AgentTeam(outcome) => Self::AgentTeam(outcome),
    OperationOutcome::SetDefaultAgentProfile => Self::DefaultAgentProfileChanged,
    OperationOutcome::ForkSession => Self::SessionForked,
    OperationOutcome::SwitchActiveLeaf => Self::ActiveLeafSwitched,
    OperationOutcome::Export(outcome) => match outcome.path { /* ... */ },
}
```

Adapters should use the same closed-enum discipline, narrowed to one expected variant:

```rust
let outcome = session
    .run(CodingAgentOperation::Prompt(prompt_options))
    .await
    .map_err(CliError::from)?;
let prompt_outcome = match outcome {
    CodingAgentOperationOutcome::Prompt(outcome) => outcome,
    _ => unreachable!("prompt operation returned a different public outcome"),
};
```

Do not use `_ => Ok(())`, discard an outcome unchecked, or turn unexpected outcomes into a new user-visible error string. An impossible variant is a programming invariant; existing adapter-visible errors still come from `run` or the extracted domain outcome.

### Family 2: JSON And Print Preserve Their Projection Shells

**Apply to:** `protocol/json_mode.rs`, `print_mode.rs`, and their three integration suites.

**JSON streaming analog:** `crates/pi-coding-agent/src/protocol/json_mode.rs:87-117,188-199`

```rust
let mut session = open_json_coding_session(&options).await?;
let mut receiver = session.subscribe_product_events();
let prompt_options = PromptTurnOptions::from_prompt_run_options(options);
let (done_tx, mut done_rx) = tokio::sync::oneshot::channel();

tokio::spawn(async move {
    let _ = done_tx.send(/* replace only this broad call */);
});

loop {
    tokio::select! {
        event = receiver.recv() => match event {
            Ok(event) => push_product_protocol_events(stdout, adapter, &event)?,
            Err(CodingSessionError::Cancelled) => {
                return done_rx.await.map_err(|_| CodingSessionError::Cancelled)?;
            }
            Err(error) => return Err(error),
        },
        result = &mut done_rx => {
            drain_json_events(&mut receiver, stdout, adapter)?;
            return result.map_err(|_| CodingSessionError::Cancelled)?;
        }
    }
}
```

Keep header and `AgentStart` emission, synthetic `PromptFailed`, exit codes, stderr text, receiver creation before execution, and the completion-time `try_recv` drain unchanged. Only the spawned call becomes `run(Prompt)` plus expected-outcome extraction.

**Print lifecycle/projection analog:** `crates/pi-coding-agent/src/print_mode.rs:108-145,159-191,201-215`

```rust
let mut session =
    open_print_coding_session(session_options, options.session_target.as_ref()).await?;
let prompt_options = PromptTurnOptions::from_prompt_run_options(options);
let outcome = /* run Prompt and extract Prompt outcome */;
Ok(outcome)

fn print_text_from_prompt_outcome(outcome: PromptTurnOutcome) -> Result<String, CliError> {
    match outcome {
        PromptTurnOutcome::Success { final_text, .. } => Ok(final_text),
        PromptTurnOutcome::Aborted { reason, .. } => Err(CliError::SessionFailure(reason)),
        PromptTurnOutcome::Failed { error, .. } => Err(print_cli_error_from_prompt_error(error)),
    }
}
```

Keep persistent and transient branches separate. `open_print_coding_session`, target rejection, continue/fork lifecycle behavior, and `print_cli_error_from_prompt_error` remain adapter-owned.

**Verification analogs:**

- `tests/json_mode.rs:35-64,125-180` checks serialized lifecycle, provider failure, and enabled-session behavior.
- `tests/print_mode.rs:103-160` checks returned text and disabled-session no-file behavior.
- `tests/session_print_mode.rs:35-82` checks `session.json`, semantic `events.jsonl` facts, and output parity.

### Family 3: RPC Background Operations Replace The Pinned Future, Not The Protocol Loop

**Apply to:** `protocol/rpc/prompt.rs`, `protocol/rpc/state.rs`, `tests/rpc_mode.rs`, `tests/protocol_sessions.rs`.

**Adapter-owned state analog:** `crates/pi-coding-agent/src/protocol/rpc/state.rs:61-85`

```rust
pub(super) struct CodingRunningPrompt {
    pub(super) events: mpsc::Receiver<RpcQueuedProductEvent>,
    pub(super) done: oneshot::Receiver<CodingOperationTaskResult>,
    pub(super) control: Option<PromptControlHandle>,
    pub(super) operation_kind: OperationKind,
    pub(super) adapter: RpcCodingEventAdapter,
    pub(super) product_event_replay: Option<ProductEventReplayHandle>,
    pub(super) adapter_applied_sequence: ProductEventSequence,
    pub(super) replayed_through_sequence: ProductEventSequence,
    pub(super) events_closed: bool,
    pub(super) idempotency_key: Option<OperationIdempotencyKey>,
}

pub(super) struct CodingOperationTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) session_root: Option<PathBuf>,
    pub(super) outcome: CodingOperationOutcome,
}
```

These fields and their update order are protocol state, not compatibility scaffolding. Retain them.

**Pinned future/select/drain analog:** `crates/pi-coding-agent/src/protocol/rpc/prompt.rs:879-940,984-1069,1091-1110`

```rust
let control = session.prompt_control_handle()?;
let mut receiver = session.subscribe_product_events();
let (event_tx, event_rx) = RpcProductEventQueue::new();
let (done_tx, done_rx) = oneshot::channel();
let product_event_replay = session.product_event_replay_handle();

write_rpc_response(writer, RpcResponse::success(id, "prompt", None)).await?;
write_json_line(writer, &ProtocolEvent::AgentStart).await?;

tokio::spawn(async move {
    let outcome = {
        let mut prompt = Box::pin(/* session.run(CodingAgentOperation::Prompt(...)) */);
        let mut product_event_forwarding_open = true;
        loop {
            tokio::select! {
                event = receiver.recv(), if product_event_forwarding_open => { /* unchanged */ }
                outcome = &mut prompt => {
                    break outcome.map_err(CliError::from);
                }
            }
        }
    };

    drain_product_events_to_rpc_queue(&mut receiver, &event_tx).await;
    let _ = done_tx.send(CodingOperationTaskResult { session, session_root, outcome: /* existing envelope */ });
});
```

For prompt, agent, team, and delegation approval, retain:

- response-before-task/event ordering;
- bounded `RpcProductEventQueue` and overflow recovery;
- prompt control outside the operation;
- every `tokio::select!` branch and guard;
- post-completion drain;
- idempotency key lifecycle;
- replay/applied sequence cursors;
- owner restoration through `result.session`;
- the existing adapter-local `CodingOperationOutcome` envelope unless a minimal local type adjustment is required.

Extract the public variant inside the spawned task before constructing the existing local result. Do not pass an unchecked `CodingAgentOperationOutcome` into the RPC finisher.

**Behavior tests:**

- `tests/rpc_mode.rs:898-948` and `1440-1503` preserve agent/team response fields and event families.
- `tests/rpc_mode.rs:2125-2200` preserves response-first and live event delivery before provider completion.
- `tests/rpc_mode.rs:2203-2260` and `2265-2400` preserve abort, steer, and follow-up routing.

### Family 4: RPC Mutations Preserve Wire Projection And Session Restoration

**Apply to:** `protocol/rpc/commands.rs` and RPC mutation tests.

**Success/error/event-drain analog:** `crates/pi-coding-agent/src/protocol/rpc/commands.rs:607-640,1496-1520`

```rust
match /* run(SelfHealingEdit) + expected outcome extraction */ {
    Ok(outcome) => {
        let data = rpc_self_healing_edit_data(&outcome);
        let drained = drain_product_events_to_protocol_events(&mut receiver, &mut adapter);
        self.coding_session = Some(session);
        write_rpc_response(writer, RpcResponse::success(id, "self_healing_edit", Some(data))).await?;
        write_drained_protocol_events(writer, drained).await?;
        self.mark_idempotency_complete(complete_key.as_ref());
        Ok(())
    }
    Err(error) => {
        let drained = drain_product_events_to_protocol_events(&mut receiver, &mut adapter);
        // Keep rpc_self_healing_edit_error_data and response shape unchanged.
        self.coding_session = Some(session);
        // ...
    }
}
```

Use the same pattern for profile mutation (`commands.rs:837-859`), rejection (`1025-1060`), plugin command (`1115-1159`), and reload (`1162-1203`): validate first, subscribe before mutation when events are expected, run the typed operation, exhaustively extract the variant, restore the session owner on every path, and preserve exact JSON fields/error strings/event drain ordering.

For plugin command, preserve the conditional `PluginLoad` before first command and refresh/query behavior. The runtime outcome type changes to public `CodingAgentPluginLoadOutcome`; wire rendering stays in existing RPC helpers.

### Family 5: Interactive Async Ownership, Navigation Continuity, And Guards

**Apply to:** `interactive/prompt_task.rs`, `loop.rs`, `commands.rs`, `session_actions.rs`, `event_bridge.rs`, interactive tests, and boundary guards.

**Task-owner/select analog:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:18-95,610-679`

```rust
pub(super) struct CodingPromptTaskResult {
    pub(super) session: CodingAgentSession,
    pub(super) outcome: PromptTurnOutcome,
    pub(super) completion_notice: Option<String>,
    pub(super) hydrate_transcript: bool,
}

let prompt_control = session.prompt_control_handle()?;
let mut receiver = session.subscribe_product_events();
send_ui_snapshot(&event_tx, &session);

let outcome = {
    let mut prompt = Box::pin(/* session.run(Prompt) */);
    loop {
        tokio::select! {
            control = control_rx.recv(), if controls_open => { /* abort/steer/follow-up unchanged */ }
            event = receiver.recv() => { /* forward ProductEvent unchanged */ }
            outcome = &mut prompt => { break outcome.map_err(CliError::from); }
        }
    }
}?;

while let Ok(Some(event)) = receiver.try_recv() {
    let _ = event_tx.send(PromptTaskEvent::Coding(event));
}
Ok(CodingPromptTaskResult { session, outcome, completion_notice: None, hydrate_transcript: false })
```

Apply this shell to prompt, agent, team, approval, compact, self-heal, plugin load/command, and direct branch summary. The future and typed extraction change; controls, receiver timing, snapshots, final drains, result structs, visible notices, and plugin UI-extension refresh stay.

**Async mutation ownership analog:** `crates/pi-coding-agent/src/interactive/loop.rs:1259-1302,1964-2088`

Approval already demonstrates the correct transition: resolve synchronously, `coding_session.take()`, mark the UI running, spawn a task that owns the session, then restore `result.session` in `finish_prompt`. Profile mutation and delegation rejection should join this async ownership boundary rather than call `block_on`, detach a task, or mutate local UI state before runtime success.

For rejection, preserve the current projection behavior from `loop.rs:1329-1348`:

```rust
let mut receiver = session.subscribe_product_events();
// run RejectDelegation and validate DelegationRejected
let mut bridge = CodingEventBridge::new();
let mut ui_events = Vec::new();
while let Ok(Some(event)) = receiver.try_recv() {
    ui_events.extend(bridge.handle_product_event(&event));
}
if ui_events.is_empty() {
    ui_events.push(UiEvent::SystemNotice { text: /* unchanged fallback */ });
}
```

**Navigation analog:** `crates/pi-coding-agent/src/interactive/prompt_task.rs:1160-1282` and `interactive/loop.rs:1812-1857,1964-1996`

Direct branch summary keeps `AlwaysCreate`, returns the same owner, and sets `hydrate_transcript: false`. Summary-before-navigation must:

1. subscribe before summary;
2. run `BranchSummary` with `ReuseExisting`;
3. drain final summary events;
4. run `ForkSession` on the same mutable owner;
5. validate `SessionForked`;
6. return that same owner with `hydrate_transcript: true`;
7. let `finish_prompt` hydrate/apply the refreshed session and then restore `coding_session`.

Keep the pre-operation receiver alive across the fork. Do not reopen a new owner just to perform the live transition. Static clone/tree/hydration helpers in `session_actions.rs` remain lifecycle/query utilities; direct `/fork` and the no-owner tree fallback must be moved into an async owner path instead of continuing through `CodingAgentSession::fork_session`.

There is no current production `SwitchActiveLeaf` caller. Do not invent one. Existing tree behavior is summary-then-fork and tests assert a forked session.

**Projection continuity:** `crates/pi-coding-agent/src/interactive/event_bridge.rs:144-185` remains unchanged. Product events continue through `CodingEventBridge`; sequence deduplication/snapshot recovery remains in the existing interactive projection stack.

**Navigation tests:**

- `tests/interactive_sessions.rs:108-186` checks selected-leaf fork and summary-before-fork behavior.
- `tests/interactive_mode.rs:526-560` checks direct `/fork` creates the visible/persistent new session.
- Add the Wave 0 direct `/branch-summary` test and profile mutation/persistence test in the same scripted harness style.
- Adapt the existing delegation rejection boundary test when its handler becomes async; keep the fallback-notice assertion.

**Source-guard analog:** `crates/pi-coding-agent/tests/product_runtime_boundary_guards.rs:296-340,940-959`

```rust
for relative_root in ["src/interactive", "src/protocol", "src/print_mode.rs"] {
    collect_source_violations(
        scan.repo_root(),
        &scan.crate_root.join(relative_root),
        &[],
        &mut violations,
        |line| /* narrowly match session receiver broad calls or local #[allow(deprecated)] */,
    );
}
assert!(violations.is_empty(), "...\n{}", violations.join("\n"));
```

Add narrow Phase 3 checks only if they provide executable RED/GREEN gates. Match session receiver calls and production-local deprecation attributes; exclude the UI method `root.set_default_agent_profile_id(...)`. Preserve the existing bounded queue guard (`prompt.rs` must use `RpcProductEventQueue`, not unbounded channels). Parser-complete recursive hardening remains Phase 5.

## Shared Patterns

### Imports And Privacy

Production adapters import operation contracts from `crate::api`, the in-crate equivalent of downstream `pi_coding_agent::api`. Do not import `coding_session::Operation`, operation metadata, services, plugin load options, registries, or Flow nodes.

### Expected Outcome Matrix

| Operation | Required Public Outcome |
|---|---|
| `Prompt` | `CodingAgentOperationOutcome::Prompt` |
| `Compact` | `CodingAgentOperationOutcome::Compact` |
| `BranchSummary` | `CodingAgentOperationOutcome::BranchSummary` |
| `SelfHealingEdit` | `CodingAgentOperationOutcome::SelfHealingEdit` |
| `InvokeAgent` | `CodingAgentOperationOutcome::AgentInvocation` |
| `InvokeTeam` | `CodingAgentOperationOutcome::AgentTeam` |
| `PluginLoad` | `CodingAgentOperationOutcome::PluginLoad` |
| `PluginCommand` | `CodingAgentOperationOutcome::PluginCommand` |
| `SetDefaultAgentProfile` | `CodingAgentOperationOutcome::DefaultAgentProfileChanged` |
| `ApproveDelegation` | `CodingAgentOperationOutcome::DelegationApproved` |
| `RejectDelegation` | `CodingAgentOperationOutcome::DelegationRejected` |
| `ForkSession` | `CodingAgentOperationOutcome::SessionForked` |

### Event And Control Ordering

- Subscribe before submitting the operation.
- Obtain `PromptControlHandle` before running prompt/agent work where currently required.
- Keep `tokio::select!` branches and guards in their current scope.
- Drain `try_recv` after operation completion before reporting final task completion.
- Do not move abort, steer, or follow-up into `CodingAgentOperation`.
- Preserve RPC response-before-events behavior and interactive task completion ordering.

### Session Owner Mutation

- RPC/interactive background tasks own `CodingAgentSession` and return it in their task result.
- Restore the owner on success and error paths.
- Navigation mutates the same owner via `run(ForkSession)`.
- Refresh snapshot/hydration after persistent transitions; do not recreate runtime/plugin/profile state through a fresh open unless the existing lifecycle path explicitly requires it.

### Adapter-Owned Projection

- JSON keeps JSONL header/events/exit/stderr projection.
- Print keeps final-text and `CliError` mapping.
- RPC keeps command names, response data, errors, idempotency, queues, replay cursors, and protocol event mapping.
- Interactive keeps transcript/menu/dialog/footer/plugin-extension projection and fallback notices.

### Verification Sampling

- After each task: run the exact adapter test(s), scoped source audit, and `cargo check -p pi-coding-agent` when types/ownership change.
- After each plan: run the complete touched integration suite plus `cargo check -p pi-coding-agent`.
- Phase gate: `cargo fmt --check`, `cargo test -p pi-coding-agent`, `cargo check -p pi-coding-agent`, scoped source audits, and `git diff --check`.

## No Analog Needed

No new production files, crates, framework abstractions, event caches, control handles, or persistence formats are required. The closest analog for every change is the target adapter's existing behavioral shell combined with the completed Phase 2 public operation contract.

## Metadata

**Analog search scope:** `crates/pi-coding-agent/src/coding_session`, JSON/print adapters, `src/protocol/rpc`, `src/interactive`, and focused adapter integration/boundary tests

**Primary source families inspected:**

1. `coding_session/public_operation.rs` and `CodingAgentSession::run`
2. JSON/print prompt shells and projection tests
3. RPC state, pinned futures, command projections, and control/event tests
4. Interactive prompt tasks, loop ownership, event bridge, navigation, and scripted tests
5. `product_runtime_boundary_guards.rs` source-scan patterns

**Pattern extraction date:** 2026-07-12
