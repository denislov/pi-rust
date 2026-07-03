# Durable Delegation Confirmation Policy Design

## Purpose

`pi-rust` supports policy-gated model-requested delegation through `delegate_agent` and `delegate_team`, including confirmation-held requests that can be listed, approved, or rejected through RPC and interactive slash commands.

Before this design, a confirmation-held request lived only in `CodingAgentSession` memory. If the process exited or the session was reopened, the request was lost even though the session event log contained the prompt and tool call that produced it. That weakened auditability and blocked later recursive budget and capability work.

This design makes pending delegation confirmations part of the Rust-native typed session event log. The session owner should derive pending confirmations from durable request and resolution events rather than from an independent sidecar state file.

## Goals

- Persist confirmation-held delegation requests in the typed session event log for persistent sessions.
- Restore unresolved pending confirmations when opening or reopening a persistent `CodingAgentSession`.
- Persist approval and rejection decisions as durable resolution events.
- Keep non-persistent sessions on the current in-memory behavior.
- Keep RPC and interactive list/approve/reject commands using the same `CodingAgentSession` owner APIs.
- Preserve the existing `CodingAgentEvent` lifecycle for adapters:
  - `DelegationConfirmationRequired`
  - `DelegationApproved`
  - `DelegationRejected`
  - `DelegationStarted`
  - `DelegationCompleted`
  - `DelegationFailed`
- Keep confirmed child execution bounded by the same owner-created `AgentInvocationFlow` and `AgentTeamFlow` paths used today.

## Non-Goals

- Do not add richer interactive confirmation prompts in this stage.
- Do not implement recursive child budget accounting in this stage.
- Do not add automatic expiry, policy migration, or user preference storage for confirmations.
- Do not persist pending confirmation state in the session manifest or a sidecar JSON file.
- Do not make TypeScript session JSONL compatible with delegation confirmations.
- Do not expose raw session storage or runtime internals to plugins.
- Do not allow approval of requests that cannot be represented as typed `DelegationRequest` values.

## Pre-Implementation State

Before this slice, `CodingAgentSession` owned:

- `pending_delegation_confirmations: Vec<PendingDelegationConfirmationState>`
- `pending_delegation_confirmations()`
- `approve_delegation_confirmation(operation_id, tool_call_id)`
- `reject_delegation_confirmation(operation_id, tool_call_id, reason)`

When `PromptTurnContext` authorized a queued request as `RequiresConfirmation`, the owner pushed an in-memory pending state and emitted `DelegationConfirmationRequired`. Approval removed the pending state, emitted `DelegationApproved`, and executed the held child flow. Rejection removed the pending state and emitted `DelegationRejected`.

The Rust-native session log already persisted prompt transcript operations, branch summaries, manual compaction, plugin load results, and active leaf changes. It did not yet have durable delegation confirmation request or resolution events.

## Design

### Source of Truth

The event log is the durable source of truth for persistent sessions.

`CodingAgentSession` may keep an in-memory pending queue for fast access, but that queue is derived from:

1. durable confirmation request events;
2. durable confirmation resolution events;
3. the current process's newly created unresolved confirmations.

The session manifest should not store pending confirmation lists. The manifest remains session identity and navigation metadata, not workflow state.

### Event Model

Add typed session events for the durable confirmation lifecycle:

```text
delegation.confirmation.requested
delegation.confirmation.approved
delegation.confirmation.rejected
```

`delegation.confirmation.requested` stores the complete typed request needed to reconstruct `PendingDelegationConfirmationState`:

- `source_operation_id`
- `turn_id`
- `tool_call_id`
- `requesting_profile_id`
- `target_kind`
- `target_id`
- `task`
- `reason`
- a non-sensitive child prompt runtime seed:
  - prompt mode;
  - model metadata with request headers stripped;
  - system prompt when present;
  - max turns when present;
  - runtime tool names;
  - whether built-in tools were registered;
  - thinking level when present;
  - tool execution mode when present;
  - session display name when present;
  - parent delegation depth.

The durable event must not store `RuntimeSnapshot`, provider credentials, model request headers, resolved tool closures, plugin service internals, loaded runtime instances, or raw `CodingAgentSession` state. After reopen, approval rebuilds child `PromptTurnOptions` from the persisted runtime seed plus the current session owner configuration, settings, auth, plugin service, and profile registry. API keys are resolved from current auth at approval time, model headers are not restored from the session log, and built-in tools are restored by name when still available. This means restored approval uses the current available runtime configuration, while the delegated target and task remain the originally requested target and task.

`delegation.confirmation.approved` stores:

- `source_operation_id`
- `tool_call_id`
- the operation id of the approval workflow.

The event envelope `created_at` is the approval timestamp. No separate `approved_at` field is needed.

`delegation.confirmation.rejected` stores:

- `source_operation_id`
- `tool_call_id`
- `reason`.

The exact Rust type names can be shorter, but the JSON event kind names should be explicit and stable.

### Replay-Derived Pending Queue

Session replay should derive pending confirmations by folding confirmation request and resolution events:

- request adds a pending item keyed by `(source_operation_id, tool_call_id)`;
- approval removes the matching pending item;
- rejection removes the matching pending item;
- duplicate unresolved request for the same key records a replay warning and keeps the first item;
- resolution without a matching request records a replay warning and otherwise has no effect.

`SessionReplay` should expose the derived pending confirmation list for `CodingAgentSession::from_services()`. The replay output can use an internal pending type that is converted into `PendingDelegationConfirmationState`.

### Session Owner Behavior

For persistent sessions:

- opening a session reconstructs `pending_delegation_confirmations` from replay;
- creating a confirmation-held request appends a durable request event before exposing it through list/approve/reject APIs;
- approving a pending confirmation appends a durable approved event before executing child work;
- rejecting a pending confirmation appends a durable rejected event before emitting `DelegationRejected`;
- if writing the durable request or resolution event fails, the owner must not mutate the in-memory pending queue as if the durable change succeeded.

For non-persistent sessions:

- keep the existing in-memory queue;
- do not synthesize durable events;
- keep public API behavior the same as today.

### Approval and Child Execution

Approval remains an owner operation:

1. resolve pending confirmation by `(operation_id, tool_call_id)`;
2. begin the appropriate owner operation kind;
3. durably record the approval decision for persistent sessions;
4. remove the pending item from memory;
5. emit `DelegationApproved`;
6. execute the child `AgentInvocationFlow` or `AgentTeamFlow`;
7. emit `DelegationStarted` and either `DelegationCompleted` or `DelegationFailed` through the existing path.

If child execution fails after the approval is durably recorded, the approval remains recorded and the failure is represented by `DelegationFailed`. This reflects the real lifecycle: the user approved the request, and the execution failed.

### Rejection

Rejection remains an owner mutation:

1. resolve pending confirmation by `(operation_id, tool_call_id)`;
2. normalize an empty rejection reason to `delegation rejected by user`;
3. durably record the rejection decision for persistent sessions;
4. remove the pending item from memory;
5. emit `DelegationRejected`;
6. do not execute child work.

### Error Handling

- Unknown pending confirmation returns the existing typed input error shape.
- Duplicate unresolved request keys during live execution should be rejected or ignored before mutation. The implementation should prefer preserving the first pending request and recording a diagnostic for the duplicate.
- Durable write failure must leave the pending queue unchanged.
- Replay warnings should not prevent session open unless the event log is structurally unreadable.
- If a restored pending item references a profile that no longer exists, it remains listable and rejectable. Approval should fail through the existing child flow validation and emit the current failure behavior.

### Adapter Behavior

RPC and interactive adapters should not manage durable state directly.

- `list_delegation_confirmations` and `/delegations` read from `CodingAgentSession::pending_delegation_confirmations()`.
- `approve_delegation` and `/delegation approve` call `CodingAgentSession::approve_delegation_confirmation()`.
- `reject_delegation` and `/delegation reject` call `CodingAgentSession::reject_delegation_confirmation()`.

The existing protocol and interactive lifecycle notices remain product-event driven.

### Fork, Clone, and Export

Fork and clone copy committed event-log history. Since pending confirmations become event-log state, copied sessions inherit unresolved pending confirmations present in the copied history.

This is acceptable for this stage because fork and clone already preserve historical workflow events. Later policy work can decide whether copied sessions should auto-reject or detach pending confirmations.

Export should not gain a special pending-confirmation section in this stage. Durable events may appear indirectly through transcript or diagnostics only if existing export rendering includes them.

## Testing

Add focused deterministic tests:

- event serialization keeps stable `kind` names for request, approval, and rejection;
- replay folds request into a pending confirmation;
- replay removes pending confirmation after approval;
- replay removes pending confirmation after rejection;
- replay reports duplicate unresolved requests without creating duplicate pending items;
- persistent session prompt with confirmation-required delegation can be reopened and still lists the pending confirmation;
- reopened persistent session can approve a restored pending confirmation and execute child work;
- reopened persistent session can reject a restored pending confirmation without executing child work;
- non-persistent session confirmation behavior remains in-memory and unchanged;
- RPC and interactive list/approve/reject tests continue to pass through the owner API.

Suggested verification after implementation:

```bash
cargo fmt --check
cargo test -p pi-coding-agent --test delegation_execution
cargo test -p pi-coding-agent --test rpc_mode
cargo test -p pi-coding-agent --test interactive_mode
cargo test -p pi-coding-agent
cargo check --workspace
```

## Documentation Updates

Implementation should update:

- `docs/TODO.md`
- `docs/agent-profiles.md`
- `docs/superpowers/plans/2026-07-02-agent-profile-team-slash-invocation-plan.md`

The docs should remove the current statement that pending confirmations are not restored after restart or session reopen, and replace it with the durable event-log policy.

## Open Follow-Up Work

After this stage, the remaining delegation follow-ups are:

- richer interactive confirmation prompts;
- recursive child execution and inherited budget accounting;
- capability release and richer capability naming around delegation sub-operations;
- policy migration or expiry semantics for old pending confirmations;
- copied-session pending confirmation policy refinement.
