# Product Event Contract

## Status And Scope

This document is the authoritative contract for the current `product_event` protocol family,
version `1.0`. It records the public event inventory implemented by
`pi_coding_agent::api::CodingAgentProductEvent` and the operation/outcome pairing implemented by
`CodingAgentSession::run`.

The executable inventories in `public_event.rs` and `public_operation.rs` must remain synchronized
with this document. A change to the inventory, family assignment, kind name, terminal metadata,
durability semantics, or operation/outcome pairing is a contract change and requires corresponding
tests and a protocol version decision.

This contract governs the product semantic stream consumed by print, JSON, RPC, and interactive
adapters. It does not make raw `FlowEvent`, raw `AgentEvent`, UI state, or durable `SessionEvent`
part of the public product event protocol.

## Event Envelope

A public product event has the following semantic fields:

```text
sequence
family and kind identity
typed family payload
optional operation association
optional terminal status and terminal operation metadata
durability metadata
```

Rules:

1. `sequence` is assigned by the live event service and is strictly increasing within one live
   stream.
2. Family and kind identity are defined by the typed event variant. They must not be derived from
   Rust `Debug` output.
3. `operation_id`, when present, is the stable correlation key shared by operation execution,
   product events, durable session facts, snapshots, and protocol outcomes.
4. A root operation publishes at most one normalized terminal operation event. Session write
   completion is not a substitute for the root operation terminal event.
5. Live product events are not durable merely because they were delivered. Durability is explicit
   in the event envelope.
6. Unknown required family or kind semantics fail closed. Adapters must not silently reinterpret an
   unknown event as a diagnostic or generic success.

## Families

| Family | Ownership |
|---|---|
| Session | Session lifecycle, durable write state, and committed compaction facts |
| Profile | Agent/profile selection changes |
| Agent | Agent invocation and low-level agent turn lifecycle projected to product semantics |
| Team | Agent-team lifecycle and member progress |
| Message | Assistant message and thinking stream projection |
| Tool | Tool call lifecycle and updates |
| Runtime | Runtime-local lifecycle and compaction state |
| Delegation | Delegation request, confirmation, execution, and terminal state |
| Workflow | Product workflow lifecycle and root operation terminal state |
| Diagnostic | Non-secret product diagnostics |
| Capability | Capability generation and revocation changes |

## Authoritative Event Inventory

The first column is the compatibility/internal variant name used by the executable fixture. The
second and third columns are the stable typed public family and kind identities for protocol `1.0`.

<!-- product-event-inventory:start -->
| Internal variant | Public family | Public kind |
|---|---|---|
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
| `RuntimeShutDown` | `runtime` | `shut_down` |
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

## Operation And Outcome Matrix

Every public operation accepted by `CodingAgentSession::run` has exactly one public outcome family.
This matrix does not imply that every outcome is successful; failure, abort, rejection, and
in-doubt semantics are represented by the returned error/admission result and normalized product
terminal events as applicable.

<!-- operation-outcome-matrix:start -->
| Operation | Outcome |
|---|---|
| `Prompt` | `Prompt` |
| `Compact` | `Compact` |
| `BranchSummary` | `BranchSummary` |
| `SelfHealingEdit` | `SelfHealingEdit` |
| `InvokeAgent` | `AgentInvocation` |
| `InvokeTeam` | `AgentTeam` |
| `PluginLoad` | `PluginLoad` |
| `PluginCommand` | `PluginCommand` |
| `SetDefaultAgentProfile` | `DefaultAgentProfileChanged` |
| `ApproveDelegation` | `DelegationApproved` |
| `RejectDelegation` | `DelegationRejected` |
| `ForkSession` | `SessionForked` |
| `SwitchActiveLeaf` | `ActiveLeafSwitched` |
| `ExportCurrent` | `Export` |
| `ExportCurrentHtml` | `ExportHtml` |
<!-- operation-outcome-matrix:end -->

## Durability

The current public durability states are:

```text
LiveOnly
PendingSessionWrite { operation_id }
Durable { session_id, ... }
```

`PendingSessionWrite` announces intent to persist and is not a committed fact. A corresponding
durable event is authoritative only after the session transaction commits. A skipped or failed
write remains live-only unless a separate durable recovery fact is committed.

`SessionEvent` remains the durable source of truth. There is no required one-to-one mapping between
ProductEvent and SessionEvent: streaming deltas may be live-only, and one durable transaction may
produce several product-level lifecycle events.

## Terminal Semantics

Terminal status and terminal operation metadata are normalized independently from family payloads.
For an operation-associated terminal event:

- the envelope operation id and terminal operation id must identify the same root operation;
- terminal operation kind must match the submitted operation descriptor;
- exactly one terminal status is observable for a root operation in one stream;
- a committed session-write event does not by itself terminate Prompt, Compact, or another root
  workflow;
- recovery publishes an explicit recovered/in-doubt semantic rather than synthesizing ordinary
  success.

## Adapter Boundary

Print, JSON, RPC, and interactive adapters consume product events in sequence and project them into
their own output/view models. They must not:

- consume raw Flow node ids as protocol fields;
- expose raw `AgentEvent` or compatibility event enums;
- infer event identity through debug formatting;
- persist UI state as session truth;
- silently ignore sequence gaps where a fresh snapshot is required;
- print unstructured diagnostics to machine-readable stdout.

## Versioning

Protocol family compatibility follows these rules:

```text
major change = incompatible event or envelope contract
minor change = backward-compatible optional addition
unknown required feature = fail closed
patch change = implementation detail, not a protocol field
```

The `0.2.0` architecture convergence work will define ProductEvent protocol `2.0`. Until that
contract is accepted, this document and its executable inventory remain the authoritative `1.0`
baseline used for differential tests and migration review.
