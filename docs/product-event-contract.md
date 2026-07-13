# Product Event Contract

This document records the current `pi_coding_agent::api` product-event contract. It is an
inventory of implemented behavior, not a promise that every operation already has a distinct
root terminal event.

## Envelope

`CodingAgentProductEvent` carries one sequence assigned by `EventService` before retention and
broadcast, one typed family payload, an optional operation ID, optional event terminal status,
optional root-operation terminal association, and independent durability. Live product-event
sequence is distinct from durable session-log envelope order.

The typed `event` field serializes as `{"family":"snake_case","payload":{"kind":"snake_case",...}}`.
Optional metadata serializes as `null` when absent. Transitional `family` and `kind` strings retain
their legacy spelling for Phase 7 migration, but typed enums and snake_case Serde names are the
authoritative identity.

Durability has three states:

- `live_only`: no durable fact is claimed.
- `pending_session_write`: includes the operation ID whose write has begun.
- `durable`: includes the committed session ID.

Terminal status is an event fact (`completed`, `failed`, `aborted`, or `recovered`). It does not by
itself mean the root operation completed. Tool, message, delegation, and session-write completion
events have no root-operation association. `PartialCommit` remains an attributed operation
error/outcome carrying its operation ID; it is not a product-event terminal or durability state.

## Event Inventory

| Family | Variants | Stable payload and correlation fields |
|---|---|---|
| Session | `Opened`, `WritePending`, `WriteCommitted`, `WriteSkipped`, `CompactionCompleted` | session ID; operation ID; skip reason; compaction turn, summary, first-kept message, tokens before |
| Profile | `DefaultChanged` | profile ID; operation ID absent |
| Agent | `InvocationStarted`, `InvocationCompleted`, `InvocationFailed`, `InvocationAborted`, `TurnStarted`, `ProviderRequestStarted` | operation/child/turn IDs, profile, task/result/error/reason, provider/model |
| Team | `Started`, `MemberStarted`, `MemberCompleted`, `Completed`, `Failed`, `Aborted` | operation/child IDs, team/profile IDs, task/result/error/reason |
| Message | `Started`, `Delta`, `ThinkingDelta`, `Completed` | operation/turn IDs, optional message ID, text/final text, usage and cost |
| Tool | `Started`, `Updated`, `Completed`, `Failed` | operation/turn/tool-call IDs, name, arguments/update/summary/error message |
| Runtime | `CompactionCompleted` | operation/turn IDs, summary, first-kept message, tokens before |
| Delegation | `Requested`, `Rejected`, `Approved`, `ConfirmationRequired`, `Started`, `Completed`, `Failed` | operation/turn/tool-call/child IDs, requester/target, target kind, task/reason/result/error |
| Workflow | `SelfHealingEditStarted`, `SelfHealingEditRepairAttempted`, `SelfHealingEditCompleted`, `SelfHealingEditFailed`, `PromptStarted`, `PromptCompleted`, `PromptFailed`, `PromptAborted`, `OperationRecovered` | operation/turn/recovery IDs, edit/check payload, result/error/reason |
| Diagnostic | `Diagnostic` | optional operation ID and message |
| Capability | `Changed` | generation and revocation policy; operation ID absent |

The inventory is exactly 45 variants with the source-order family distribution
`5/1/6/6/4/4/1/7/9/1/1`. `SessionOpened`, `DefaultChanged`, and `CapabilityChanged` have no
operation identity. `Diagnostic` may or may not have one. Absence is valid and must not be replaced
with a sentinel ID.

The authoritative source-order identity table is kept in sync by boundary tests:

<!-- product-event-inventory:start -->
| Internal variant | Public family | Public kind |
| `SessionOpened` | `session` | `opened` |
| `SessionWritePending` | `session` | `write_pending` |
| `SessionWriteCommitted` | `session` | `write_committed` |
| `SessionWriteSkipped` | `session` | `write_skipped` |
| `SessionCompactionCompleted` | `session` | `compaction_completed` |
| `DefaultAgentProfileChanged` | `profile` | `default_changed` |
| `AgentInvocationStarted` | `agent` | `invocation_started` |
| `AgentInvocationCompleted` | `agent` | `invocation_completed` |
| `AgentInvocationFailed` | `agent` | `invocation_failed` |
| `AgentInvocationAborted` | `agent` | `invocation_aborted` |
| `AgentTurnStarted` | `agent` | `turn_started` |
| `ProviderRequestStarted` | `agent` | `provider_request_started` |
| `AgentTeamStarted` | `team` | `started` |
| `AgentTeamMemberStarted` | `team` | `member_started` |
| `AgentTeamMemberCompleted` | `team` | `member_completed` |
| `AgentTeamCompleted` | `team` | `completed` |
| `AgentTeamFailed` | `team` | `failed` |
| `AgentTeamAborted` | `team` | `aborted` |
| `AssistantMessageStarted` | `message` | `started` |
| `AssistantMessageDelta` | `message` | `delta` |
| `AssistantThinkingDelta` | `message` | `thinking_delta` |
| `AssistantMessageCompleted` | `message` | `completed` |
| `ToolCallStarted` | `tool` | `started` |
| `ToolCallUpdated` | `tool` | `updated` |
| `ToolCallCompleted` | `tool` | `completed` |
| `ToolCallFailed` | `tool` | `failed` |
| `RuntimeCompactionCompleted` | `runtime` | `compaction_completed` |
| `DelegationRequested` | `delegation` | `requested` |
| `DelegationRejected` | `delegation` | `rejected` |
| `DelegationApproved` | `delegation` | `approved` |
| `DelegationConfirmationRequired` | `delegation` | `confirmation_required` |
| `DelegationStarted` | `delegation` | `started` |
| `DelegationCompleted` | `delegation` | `completed` |
| `DelegationFailed` | `delegation` | `failed` |
| `SelfHealingEditStarted` | `workflow` | `self_healing_edit_started` |
| `SelfHealingEditRepairAttempted` | `workflow` | `self_healing_edit_repair_attempted` |
| `SelfHealingEditCompleted` | `workflow` | `self_healing_edit_completed` |
| `SelfHealingEditFailed` | `workflow` | `self_healing_edit_failed` |
| `PromptStarted` | `workflow` | `prompt_started` |
| `PromptCompleted` | `workflow` | `prompt_completed` |
| `PromptFailed` | `workflow` | `prompt_failed` |
| `PromptAborted` | `workflow` | `prompt_aborted` |
| `OperationRecovered` | `workflow` | `operation_recovered` |
| `Diagnostic` | `diagnostic` | `diagnostic` |
| `CapabilityChanged` | `capability` | `changed` |
<!-- product-event-inventory:end -->

## Root Terminal Associations

The current five mappings are:

- `PromptCompleted`, `PromptFailed`, `PromptAborted` -> `Prompt`.
- `Session.CompactionCompleted` -> `Compact`.
- `SelfHealingEditCompleted`, `SelfHealingEditFailed` -> `SelfHealingEdit`.
- `Agent.InvocationCompleted`, `Agent.InvocationFailed`, `Agent.InvocationAborted` -> `AgentInvocation`.
- `Team.Completed`, `Team.Failed`, `Team.Aborted` -> `AgentTeam`.

`Runtime.CompactionCompleted`, message/tool/delegation terminal events, session write completion,
and `OperationRecovered` deliberately have no root-operation association today.

## Operation And Outcome Matrix

Categories mean:

- `root-terminal-associated`: current events identify the root operation through one of the five mappings above.
- `synchronous/eventless`: the operation returns an outcome without a distinct root terminal event.
- `currently-unassociated`: related events can be emitted, but none distinctly associates terminal state with this root operation.

<!-- operation-outcome-matrix:start -->
| Operation variant | Outcome variant | Category | Current terminal evidence |
|---|---|---|---|
| `Prompt` | `Prompt` | `root-terminal-associated` | `PromptCompleted`, `PromptFailed`, `PromptAborted` |
| `Compact` | `Compact` | `root-terminal-associated` | `Session.CompactionCompleted` |
| `BranchSummary` | `BranchSummary` | `currently-unassociated` | Uses prompt workflow events; no distinct branch-summary root association |
| `SelfHealingEdit` | `SelfHealingEdit` | `root-terminal-associated` | `SelfHealingEditCompleted`, `SelfHealingEditFailed` |
| `InvokeAgent` | `AgentInvocation` | `root-terminal-associated` | `Agent.InvocationCompleted`, `Agent.InvocationFailed`, `Agent.InvocationAborted` |
| `InvokeTeam` | `AgentTeam` | `root-terminal-associated` | `Team.Completed`, `Team.Failed`, `Team.Aborted` |
| `PluginLoad` | `PluginLoad` | `currently-unassociated` | Diagnostics/capability changes may emit; no plugin root terminal association |
| `PluginCommand` | `PluginCommand` | `synchronous/eventless` | No distinct root terminal event |
| `SetDefaultAgentProfile` | `DefaultAgentProfileChanged` | `synchronous/eventless` | Profile change is metadata-only, not a root terminal association |
| `ApproveDelegation` | `DelegationApproved` | `currently-unassociated` | Delegation events are event-terminal only |
| `RejectDelegation` | `DelegationRejected` | `synchronous/eventless` | Rejection event is not a root terminal association |
| `ForkSession` | `SessionForked` | `synchronous/eventless` | Navigation returns its typed outcome without a root terminal event |
| `SwitchActiveLeaf` | `ActiveLeafSwitched` | `synchronous/eventless` | Navigation returns its typed outcome without a root terminal event |
| `ExportCurrent` | `Export` | `synchronous/eventless` | Export returns data without a root terminal event |
| `ExportCurrentHtml` | `ExportHtml` | `synchronous/eventless` | Export returns a path without a root terminal event |
<!-- operation-outcome-matrix:end -->

Association closure is reserved for Phase 9. The matrix must not be used to infer terminal events
that the runtime does not currently emit.
