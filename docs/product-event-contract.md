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
stream_id
sequence
family and kind identity
typed family payload
optional operation association
optional parent/root operation lineage
optional session association from an event-owned durable/session fact
optional terminal status and terminal operation metadata
durability metadata
delivery class (`data`, `terminal`, `control`, or `recovery`)
```

Rules:

- `stream_id` is an opaque identity created once per runtime; it is independent
  from `session_id` and must equal the `stream_id` in every snapshot cursor and
  RPC `get_state.data.eventStreamId` produced by that runtime;
- root operations expose `root_operation_id == operation_id` and no parent;
- child operations expose their direct parent and the stable root across nested lineage;
- session association is absent unless the event itself carries the session fact; consumers must
  not infer it from whichever session happens to be current when an event is delivered;

1. `sequence` is assigned by the live event service and is strictly increasing within one
   `stream_id`; reconnect rejects a cursor from another stream even when its sequence is otherwise
   valid.
2. Family and kind identity are defined by the typed event variant. They must not be derived from
   Rust `Debug` output.
3. `operation_id`, when present, is the stable correlation key shared by operation execution,
   product events, durable session facts, snapshots, and protocol outcomes.
4. Every submitted operation declares one terminal publication policy.
   `ProductEvent` policy publishes exactly one normalized root terminal event.
   `OutcomeAcknowledgement` policy publishes no synthetic terminal event and
   remains terminal in the client snapshot until its exact outcome
   acknowledgement is accepted. Session write completion is not a substitute
   for either contract.
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

The first column is the normalized owner-event identity used by the executable fixture. It may come
from a family-local owner enum such as `SessionWriteEvent`, not from one global compatibility enum.
The second and third columns are the stable typed public family and kind identities for protocol
`2.0`.

<!-- product-event-inventory:start -->
| Owner event | Public family | Public kind |
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
| `SelfHealingEditAborted` | `workflow` | `self_healing_edit_aborted` |
| `PromptStarted` | `workflow` | `prompt_started` |
| `PromptCompleted` | `workflow` | `prompt_completed` |
| `PromptFailed` | `workflow` | `prompt_failed` |
| `PromptAborted` | `workflow` | `prompt_aborted` |
| `OperationRecovered` | `workflow` | `operation_recovered` |
| `Diagnostic` | `diagnostic` | `diagnostic` |
| `CapabilityChanged` | `capability` | `changed` |
| `SessionWriteFailed` | `session` | `write_failed` |
| `ToolCallAuthorizationRequired` | `tool` | `authorization_required` |
| `ToolCallAuthorizationApproved` | `tool` | `authorization_approved` |
| `ToolCallAuthorizationDenied` | `tool` | `authorization_denied` |
| `ToolCallAuthorizationCancelled` | `tool` | `authorization_cancelled` |
<!-- product-event-inventory:end -->

The first column is a normalized owner-event name, not a Rust enum layout. The
executable inventory composes owner-local lifecycle enums, the closed
`PromptStreamEvent` union, and the single-purpose `SessionCompactionEvent` and
`RecoveryEvent` structs. There is no centralized `CodingAgentEvent` enum.
Reintroducing one global semantic event bucket or an internal-to-public mapping
is a boundary violation.

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
| `SetDefaultAgentProfile` | `DefaultAgentProfileChanged` |
| `ApproveDelegation` | `DelegationApproved` |
| `RejectDelegation` | `DelegationRejected` |
| `ForkSession` | `SessionForked` |
| `SwitchActiveLeaf` | `ActiveLeafSwitched` |
| `SetSessionTreeLabel` | `SessionTreeLabelChanged` |
| `ExportCurrent` | `Export` |
| `ExportCurrentHtml` | `ExportHtml` |
<!-- operation-outcome-matrix:end -->

## Durability

The current public durability states are:

```text
LiveOnly
PendingSessionWrite { operation_id }
Durable { session_id }
DerivedFromSession { session_id, source_operation_id, recovery_id }
PersistenceUncertain { operation_id }
PersistenceFailed { operation_id, reason }
```

`PendingSessionWrite` announces intent to persist and is not a committed fact. A corresponding
durable event is authoritative only after the session transaction commits. `WriteSkipped` means no
write was attempted. `WriteFailed` carries either `definite` or `uncertain`: definite failure uses
`PersistenceFailed`, while an append/manifest/reopen path that may already have written data uses
`PersistenceUncertain`.

`DerivedFromSession` identifies a ProductEvent projected from an already committed durable fact;
startup recovery uses the original operation ID and recovery marker as its source reference.
`PersistenceUncertain` means part of the transaction may already be durable and clients must not
interpret the associated failure as a clean rollback.
`PersistenceFailed` means the owner established that the write did not commit; it must not be used
for an I/O path where a partial append cannot be excluded.

`SessionEvent` remains the durable source of truth. There is no required one-to-one mapping between
ProductEvent and SessionEvent: streaming deltas may be live-only, and one durable transaction may
produce several product-level lifecycle events.

## Pressure And Delivery Classes

Every envelope declares one delivery class:

- `data`: ordinary lifecycle, streaming, tool, message, diagnostic, and local terminal updates;
- `terminal`: an authoritative root terminal operation event;
- `control`: capability-generation and runtime-shutdown projections;
- `recovery`: a durable recovery projection.

All classes use one bounded sequence and retained window. Publishers do not block on slow clients.
Any observed sequence loss fails closed: incremental replay is rejected and the client must obtain a
fresh snapshot before resuming. The runtime never selectively replays terminal/control/recovery
events across a missing data prefix, because doing so would expose terminal facts without their
required state boundary. Control commands and cancellation authority use their own bounded control
paths; the ProductEvent control class is projection only.

## Terminal Semantics

Terminal status and terminal operation metadata are normalized independently from family payloads.
For an operation-associated terminal event:

- the envelope operation id identifies the admitted root operation whose descriptor authorized the
  terminal evidence;
- terminal operation kind must match the submitted operation descriptor;
- terminal operation metadata is derived from the admitted operation kind plus exact permitted root
  evidence, never from the event variant alone;
- Prompt terminal payload, local terminal status, partial-commit durability, and root evidence are
  constructed by `PromptEvent`; Prompt Flow context stores only idempotent completion state and does
  not cache a second terminal event;
- Agent invocation and Team terminal evidence are constructed by their owner events. Team
  cancellation publishes `agent_team_aborted`; it is not normalized as a generic team failure;
- exactly one terminal status is observable for a root operation in one stream;
- a committed session-write event does not by itself terminate Prompt, Compact, or another root
  workflow;
- tool, message, delegation, and session-write events may expose a local terminal status while
  keeping terminal operation metadata absent;
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

The `0.2` prerelease train establishes ProductEvent protocol `2.0`. This
document and its executable inventory are the authoritative `2.0` contract.
The durable session writer remains independently versioned at `1`.
