# Product Event Family Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Start Stage 2 of the operation runtime reference architecture by giving the existing flat `CodingAgentEvent` stream an internal product-event family classification while preserving current adapter-visible behavior.

**Architecture:** Keep `CodingAgentEvent` as the public adapter stream for this stage. Add crate-internal classification metadata on the enum itself so adapters, protocol guards, and later `ProductEvent` wrappers can reason in family/status terms without changing JSON/RPC/interactive event payloads. Normalize event family, optional operation correlation, and terminal status first; defer a new `ProductEvent` wrapper until existing adapters can consume the classification safely.

**Tech Stack:** Rust 2024, `pi-coding-agent`, existing `CodingAgentEvent`/protocol adapter tests, `cargo test` focused crate checks.

---

### Task 1: Add Internal Product Event Classification Contract

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing family classification tests**

Add these tests at the bottom of `crates/pi-coding-agent/src/coding_session/event.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn profile_id(value: &str) -> ProfileId {
        ProfileId::new(value.to_owned()).expect("valid profile id")
    }

    #[test]
    fn coding_agent_events_report_internal_product_families() {
        assert_eq!(
            CodingAgentEvent::SessionOpened {
                session_id: "session_1".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Session
        );
        assert_eq!(
            CodingAgentEvent::DefaultAgentProfileChanged {
                profile_id: profile_id("agent-main"),
            }
            .classification()
            .family,
            ProductEventFamily::Profile
        );
        assert_eq!(
            CodingAgentEvent::AgentInvocationStarted {
                operation_id: "op_agent".into(),
                child_operation_id: "op_child".into(),
                profile_id: profile_id("agent-main"),
                task: "review".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Agent
        );
        assert_eq!(
            CodingAgentEvent::AgentTeamStarted {
                operation_id: "op_team".into(),
                team_id: profile_id("team-main"),
                task: "review".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Team
        );
        assert_eq!(
            CodingAgentEvent::AssistantMessageDelta {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                message_id: Some("msg_1".into()),
                text: "hello".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Message
        );
        assert_eq!(
            CodingAgentEvent::ToolCallCompleted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_1".into(),
                name: "read".into(),
                summary: "ok".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Tool
        );
        assert_eq!(
            CodingAgentEvent::DelegationStarted {
                operation_id: "op_prompt".into(),
                turn_id: "turn_1".into(),
                tool_call_id: "tool_delegate".into(),
                requesting_profile_id: profile_id("agent-main"),
                target_kind: ProfileKind::Agent,
                target_id: profile_id("agent-helper"),
                task: "review".into(),
                child_operation_id: "op_child".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Delegation
        );
        assert_eq!(
            CodingAgentEvent::SelfHealingEditStarted {
                operation_id: "op_edit".into(),
                path: "src/lib.rs".into(),
                replacements: 1,
            }
            .classification()
            .family,
            ProductEventFamily::Workflow
        );
        assert_eq!(
            CodingAgentEvent::Diagnostic {
                operation_id: None,
                message: "notice".into(),
            }
            .classification()
            .family,
            ProductEventFamily::Diagnostic
        );
        assert_eq!(
            CodingAgentEvent::CapabilityChanged.classification().family,
            ProductEventFamily::Capability
        );
    }

    #[test]
    fn coding_agent_events_report_operation_correlation_and_terminal_status() {
        let completed_event = CodingAgentEvent::PromptCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
        };
        let completed = completed_event.classification();
        assert_eq!(completed.operation_id, Some("op_prompt"));
        assert_eq!(
            completed.terminal_status,
            Some(ProductEventTerminalStatus::Completed)
        );

        let failed_event = CodingAgentEvent::SelfHealingEditFailed {
            operation_id: "op_edit".into(),
            path: "src/lib.rs".into(),
            error: CodingSessionError::Provider {
                message: "provider failed".into(),
            },
        };
        let failed = failed_event.classification();
        assert_eq!(failed.operation_id, Some("op_edit"));
        assert_eq!(
            failed.terminal_status,
            Some(ProductEventTerminalStatus::Failed)
        );

        let aborted_event = CodingAgentEvent::AgentInvocationAborted {
            operation_id: "op_agent".into(),
            child_operation_id: "op_child".into(),
            profile_id: profile_id("agent-main"),
            reason: "cancelled".into(),
        };
        let aborted = aborted_event.classification();
        assert_eq!(aborted.operation_id, Some("op_agent"));
        assert_eq!(
            aborted.terminal_status,
            Some(ProductEventTerminalStatus::Aborted)
        );

        let progress_event = CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "hello".into(),
        };
        let progress = progress_event.classification();
        assert_eq!(progress.operation_id, Some("op_prompt"));
        assert_eq!(progress.terminal_status, None);

        let uncorrelated = CodingAgentEvent::CapabilityChanged.classification();
        assert_eq!(uncorrelated.operation_id, None);
        assert_eq!(uncorrelated.terminal_status, None);
    }
}
```

- [x] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p pi-coding-agent coding_agent_events_report_internal_product_families --lib
```

RED result: compile failed because `classification`, `ProductEventFamily`, `ProductEventTerminalStatus`, and `ProductEventClassification` did not exist.

- [x] **Step 3: Add the minimal internal classification model**

Add these crate-internal types near the top of `crates/pi-coding-agent/src/coding_session/event.rs`, after `CodingAgentEvent` imports and before the enum:

```rust
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductEventClassification<'event> {
    pub(crate) family: ProductEventFamily,
    pub(crate) operation_id: Option<&'event str>,
    pub(crate) terminal_status: Option<ProductEventTerminalStatus>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventFamily {
    Session,
    Profile,
    Agent,
    Team,
    Message,
    Tool,
    Runtime,
    Delegation,
    Workflow,
    Diagnostic,
    Capability,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventTerminalStatus {
    Completed,
    Failed,
    Aborted,
}
```

Add an `impl CodingAgentEvent` block that returns a `ProductEventClassification<'_>` for every current enum variant. The match must be exhaustive: adding a future `CodingAgentEvent` variant should require choosing a family and terminal status before it compiles.

- [x] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p pi-coding-agent coding_agent_events_report_internal_product_families --lib
cargo test -p pi-coding-agent coding_agent_events_report_operation_correlation_and_terminal_status --lib
```

GREEN result: both tests passed.

### Task 2: Verify Adapter Compatibility For The Classification Slice

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Run existing protocol adapter coverage**

Run:

```bash
cargo test -p pi-coding-agent protocol_events
cargo test -p pi-coding-agent protocol_jsonl
cargo test -p pi-coding-agent rpc_mode
cargo test -p pi-coding-agent interactive_event_bridge
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

GREEN result: all listed adapter compatibility tests passed, proving the internal classification did not alter adapter-visible protocol behavior.

- [x] **Step 2: Run operation/event focused checks**

Run:

```bash
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
cargo fmt --check
git diff --check
git status --short
```

GREEN result: all listed event/operation, crate, format, and diff hygiene checks passed.

- [x] **Step 3: Commit the classification slice**

Run:

```bash
git add crates/pi-coding-agent/src/coding_session/event.rs docs/TODO.md docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md
git commit -m "feat: classify coding agent product events"
```

Expected: one focused commit containing the Stage 2 classification model and verification notes.

### Task 3: Prepare The ProductEvent Wrapper Cut

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Record the next Stage 2 boundary after classification lands**

After Task 1 and Task 2 are committed, update `docs/TODO.md` to state that Stage 2 has an internal family/status classifier and that the next cut is a non-public `ProductEvent` wrapper or EventService publication boundary.

- [x] **Step 2: Keep wrapper implementation out of this commit**

Do not introduce `ProductEvent` publication or adapter migration in the classification commit. The first Stage 2 commit must remain behavior-preserving and easy to revert.

Completed by keeping the classification commit focused and starting this wrapper boundary in a later commit.

### Task 4: Add Internal ProductEvent Wrapper And Live Sequence Boundary

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing wrapper and EventService boundary tests**

Added `product_event_wrapper_owns_compatibility_event_and_metadata` to require a non-public `ProductEvent` wrapper to own the compatibility event plus sequence, family, operation id, terminal status, and durability. Added `event_service_wraps_emitted_events_with_sequence_and_preserves_compatibility_receiver` to require `EventService::emit()` to allocate strictly increasing live sequence values while still broadcasting the original `CodingAgentEvent` stream to existing receivers.

Verification:

```bash
cargo test -p pi-coding-agent product_event_wrapper_owns_compatibility_event_and_metadata --lib
```

RED result: compile failed because `ProductEvent`, `ProductEventSequence`, and `ProductEventDurability` did not exist, and `EventService::emit()` still returned `()`.

- [x] **Step 2: Add the minimal internal wrapper and live sequence publisher**

Added crate-internal `ProductEvent`, `ProductEventSequence`, and `ProductEventDurability::LiveOnly`. `ProductEvent::from_compat_event()` now derives owned metadata from the existing `CodingAgentEvent::classification()` contract. `EventService` now owns a shared atomic live sequence counter, returns the internal `ProductEvent` from `emit()`, and broadcasts `product_event.compatibility_event().clone()` so current adapters remain unchanged.

Verification:

```bash
cargo test -p pi-coding-agent product_event_wrapper_owns_compatibility_event_and_metadata --lib
cargo test -p pi-coding-agent event_service_wraps_emitted_events_with_sequence_and_preserves_compatibility_receiver --lib
cargo test -p pi-coding-agent event_service --lib
```

GREEN result: the wrapper, EventService boundary, and full event_service tests passed.

- [x] **Step 3: Verify adapter compatibility and crate hygiene**

```bash
cargo test -p pi-coding-agent protocol_events
cargo test -p pi-coding-agent interactive_event_bridge
cargo check -p pi-coding-agent
cargo fmt --check
git diff --check
git status --short
```

GREEN result: protocol and interactive adapter compatibility tests passed, `cargo check -p pi-coding-agent` passed, and final format/diff hygiene was clean after applying rustfmt.

### Task 5: Add Session-Write Durability Metadata

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing durability mapping test**

Added `product_event_wrapper_marks_session_write_durability` to require `ProductEvent::from_compat_event()` to classify `SessionWritePending` as `PendingSessionWrite { operation_id }`, classify `SessionWriteCommitted` as `Durable { session_id }`, and keep `SessionWriteSkipped` as `LiveOnly` for the current non-persistent/no-write path.

Verification:

```bash
cargo test -p pi-coding-agent product_event_wrapper_marks_session_write_durability --lib
```

RED result: compile failed because `ProductEventDurability::PendingSessionWrite` and `ProductEventDurability::Durable` did not exist.

- [x] **Step 2: Add minimal session-write durability metadata**

Extended `ProductEventDurability` with `PendingSessionWrite { operation_id }` and `Durable { session_id }`. Added `ProductEventDurability::from_compat_event()` so `ProductEvent::from_compat_event()` derives durability from the current compatibility event without changing the adapter-visible `CodingAgentEvent` stream.

Verification:

```bash
cargo test -p pi-coding-agent product_event_wrapper_marks_session_write_durability --lib
cargo test -p pi-coding-agent product_event_wrapper_owns_compatibility_event_and_metadata --lib
cargo test -p pi-coding-agent event_service_wraps_emitted_events_with_sequence_and_preserves_compatibility_receiver --lib
```

GREEN result: the new durability mapping and existing wrapper/EventService boundary tests passed.

- [x] **Step 3: Run focused compatibility and hygiene checks**

```bash
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
cargo fmt --check
git diff --check
git status --short
```

GREEN result: event_service, operation, crate check, formatting, and diff hygiene checks passed.


### Task 6: Add Family-Specific ProductEventKind

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event.rs`
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing ProductEventKind wrapper test**

Added `product_event_wrapper_exposes_family_specific_kind` to require `ProductEvent::from_compat_event()` to expose a family-specific kind for representative workflow, tool, and session events, and to derive `ProductEventFamily` from that internal kind.

Verification:

```bash
cargo test -p pi-coding-agent product_event_wrapper_exposes_family_specific_kind --lib
```

RED result: compile failed because `ProductEvent` had no `kind` field, `ProductEventKind` and the family-specific kind enums did not exist, and `ProductEvent::family()` did not exist.

- [x] **Step 2: Add minimal family-specific kind model**

Added crate-internal `ProductEventKind` plus family-specific enums for session, profile, agent, team, message, tool, runtime, delegation, workflow, diagnostic, and capability events. `ProductEvent::from_compat_event()` now stores `ProductEventKind::from_compat_event(&CodingAgentEvent)`, and `ProductEvent::family()` derives the family from the kind instead of carrying a separate flat family field. Current adapters still receive the unchanged compatibility `CodingAgentEvent` stream through `EventService`.

Verification:

```bash
cargo test -p pi-coding-agent product_event_wrapper_exposes_family_specific_kind --lib
cargo test -p pi-coding-agent product_event_wrapper_owns_compatibility_event_and_metadata --lib
cargo test -p pi-coding-agent product_event_wrapper_marks_session_write_durability --lib
cargo test -p pi-coding-agent event_service_wraps_emitted_events_with_sequence_and_preserves_compatibility_receiver --lib
```

GREEN result: the new kind mapping, existing wrapper metadata, durability mapping, and EventService compatibility boundary tests passed.

- [x] **Step 3: Run focused compatibility and hygiene checks**

```bash
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
cargo fmt --check
cargo test -p pi-coding-agent protocol_events
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
git diff --check
git status --short
```

GREEN result: event_service, operation, crate check, formatting, adapter compatibility, public API smoke, and diff hygiene checks passed before committing this slice.


### Task 7: Add Internal ProductEvent Subscription Stream

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/event_service.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing ProductEvent subscription test**

Added `event_service_publishes_internal_product_events_alongside_compatibility_stream` to require one `EventService::emit()` call to publish the same internal `ProductEvent` returned by `emit()` to a crate-internal product-event receiver while preserving the existing `CodingAgentEventReceiver` compatibility stream.

Verification:

```bash
cargo test -p pi-coding-agent event_service_publishes_internal_product_events_alongside_compatibility_stream --lib
```

RED result: compile failed because `EventService::subscribe_product_events()` did not exist.

- [x] **Step 2: Add minimal internal ProductEvent receiver**

Added a second `broadcast::Sender<ProductEvent>` owned by `EventService`, a crate-internal `ProductEventReceiver`, and `EventService::subscribe_product_events()`. `EventService::emit()` now sends the cloned internal `ProductEvent` to the product stream before sending the unchanged compatibility `CodingAgentEvent` to existing subscribers. The public `CodingAgentEventReceiver` and adapter-visible event stream remain unchanged.

Verification:

```bash
cargo test -p pi-coding-agent event_service_publishes_internal_product_events_alongside_compatibility_stream --lib
cargo test -p pi-coding-agent event_service --lib
cargo check -p pi-coding-agent
```

GREEN result: the focused product-event stream test, the full event_service test set, and crate check passed.

- [x] **Step 3: Run focused compatibility and hygiene checks**

```bash
cargo fmt --check
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
cargo test -p pi-coding-agent protocol_events
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
git diff --check
git status --short
```

GREEN result: formatting, event_service, operation, crate check, adapter compatibility, public API smoke, and diff hygiene checks passed before committing this slice.


### Task 8: Route JSON Mode Through The Internal ProductEvent Stream

**Files:**
- Modify: `crates/pi-coding-agent/src/coding_session/mod.rs`
- Modify: `crates/pi-coding-agent/src/protocol/json_mode.rs`
- Modify: `crates/pi-coding-agent/src/protocol/events.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/events.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing JSON product-event adapter tests**

Added `json_mode_protocol_adapter_accepts_product_events` to require JSON protocol projection to accept a crate-internal `ProductEvent` wrapper and still emit the existing JSONL `ProtocolEvent` shape. Added `json_prompt_stream_uses_product_event_subscription_boundary` to guard that the JSON prompt streaming loop subscribes through `session.subscribe_product_events()` instead of the compatibility `session.subscribe()` stream.

Verification:

```bash
cargo test -p pi-coding-agent json_mode_protocol_adapter_accepts_product_events --lib
```

RED result: compile failed because `push_product_protocol_events` did not exist.

- [x] **Step 2: Add the minimal JSON product-event bridge**

Added a crate-internal `CodingAgentSession::subscribe_product_events()` facade over `EventService::subscribe_product_events()`. Updated JSON mode prompt streaming and drain logic to consume `ProductEventReceiver`, and added `push_product_protocol_events()` so JSON mode projects internal `ProductEvent` values through the existing `CodingProtocolEventAdapter::push_product_event()` compatibility bridge. Adapter-visible JSONL output remains unchanged.

Verification:

```bash
cargo test -p pi-coding-agent json_mode_protocol_adapter_accepts_product_events --lib
cargo test -p pi-coding-agent json_prompt_stream_uses_product_event_subscription_boundary --lib
cargo test -p pi-coding-agent --test json_mode
cargo test -p pi-coding-agent rpc_adapter_accepts_internal_product_events --lib
```

GREEN result: JSON product-event projection, JSON product subscription guard, JSON mode integration coverage, and RPC adapter product-event bridge coverage passed.


### Task 9: Route RPC Running Prompts Through The Internal ProductEvent Stream

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/rpc.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/prompt.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/state.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing RPC running-prompt boundary test**

Added `rpc_running_prompt_uses_product_event_stream_boundary` to require the shared RPC running-prompt queue to carry `ProductEvent`, require prompt/agent/team/delegation-approval running operations to subscribe through `session.subscribe_product_events()`, and require running prompt write/drain logic to project events through `RpcCodingEventAdapter::push_product_event()`.

Verification:

```bash
cargo test -p pi-coding-agent rpc_running_prompt_uses_product_event_stream_boundary --lib
```

RED result: the boundary test failed because the RPC running prompt paths still used four compatibility `session.subscribe()` calls.

- [x] **Step 2: Migrate the shared RPC running-prompt event queue**

Changed `CodingRunningPrompt.events` from `CodingAgentEvent` to crate-internal `ProductEvent`, changed `RpcLoopEvent::CodingEvent` to carry `ProductEvent`, renamed the running write helper to `write_product_event()`, and updated prompt, agent invocation, team invocation, and delegation approval running paths to subscribe through `session.subscribe_product_events()`. Existing RPC wire events still flow through the compatibility projection inside `RpcCodingEventAdapter`.

Verification:

```bash
cargo test -p pi-coding-agent rpc_running_prompt_uses_product_event_stream_boundary --lib
cargo test -p pi-coding-agent --test rpc_mode
```

GREEN result: the RPC running-prompt product-event boundary test passed, and all 36 RPC mode integration tests passed with unchanged wire behavior.


### Task 10: Route RPC Synchronous Command Drains Through ProductEvent

**Files:**
- Modify: `crates/pi-coding-agent/src/protocol/rpc/commands.rs`
- Modify: `crates/pi-coding-agent/src/protocol/rpc/events.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing RPC sync-command boundary test**

Added `rpc_sync_commands_use_product_event_stream_boundary` to require the RPC synchronous command drain paths to subscribe through `session.subscribe_product_events()` and project drained events through `RpcCodingEventAdapter::push_product_event()`.

Verification:

```bash
cargo test -p pi-coding-agent rpc_sync_commands_use_product_event_stream_boundary --lib
```

RED result: the boundary test failed because the three synchronous command drains still subscribed to the compatibility event stream.

- [x] **Step 2: Migrate the RPC synchronous command drains**

Updated self-healing edit, default-agent-profile update, and delegation rejection RPC command drains to consume `ProductEvent` from `session.subscribe_product_events()`. Removed the now-unused `RpcCodingEventAdapter::push(&CodingAgentEvent)` method and updated RPC adapter tests to construct `ProductEvent` wrappers before projection.

Verification:

```bash
cargo test -p pi-coding-agent rpc_sync_commands_use_product_event_stream_boundary --lib
cargo test -p pi-coding-agent rpc_adapter_maps_product_prompt_stream_to_protocol_events --lib
cargo test -p pi-coding-agent rpc_adapter_maps_product_failure_to_protocol_error_message --lib
cargo test -p pi-coding-agent --test rpc_mode
```

GREEN result: the RPC sync-command boundary test passed, RPC adapter product-event tests passed, and all 36 RPC mode integration tests passed with unchanged wire behavior.


### Task 11: Route Interactive Adapters Through ProductEvent

**Files:**
- Modify: `crates/pi-coding-agent/src/interactive/event_bridge.rs`
- Modify: `crates/pi-coding-agent/src/interactive/loop.rs`
- Modify: `crates/pi-coding-agent/src/interactive/prompt_task.rs`
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [x] **Step 1: Write failing interactive product-event boundary tests**

Added `coding_event_bridge_accepts_product_events` to require the UI event bridge to accept internal `ProductEvent` wrappers. Added `interactive_prompt_tasks_use_product_event_stream_boundary` to require all interactive prompt task runners to subscribe through `session.subscribe_product_events()`. Added `interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary` to guard the interactive synchronous delegation-rejection drain.

Verification:

```bash
cargo test -p pi-coding-agent coding_event_bridge_accepts_product_events --lib
cargo test -p pi-coding-agent interactive_prompt_tasks_use_product_event_stream_boundary --lib
cargo test -p pi-coding-agent interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary --lib
```

RED result: compile failed because `CodingEventBridge::handle_product_event()` did not exist, and the interactive subscription guards still expected product-event subscriptions that were not present.

- [x] **Step 2: Migrate interactive prompt tasks and bridge projection**

Added `CodingEventBridge::handle_product_event()` as a compatibility projection over `ProductEvent::compatibility_event()`. Changed `PromptTaskEvent::Coding` to carry `ProductEvent`, updated all interactive prompt task runners to subscribe through `session.subscribe_product_events()`, and changed the synchronous delegation rejection drain to use the same product-event bridge. UI mapping still lives in the existing `CodingEventBridge::handle(&CodingAgentEvent)` implementation for now.

Verification:

```bash
cargo test -p pi-coding-agent coding_event_bridge_accepts_product_events --lib
cargo test -p pi-coding-agent interactive_prompt_tasks_use_product_event_stream_boundary --lib
cargo test -p pi-coding-agent interactive_loop_sync_delegation_rejection_uses_product_event_stream_boundary --lib
cargo test -p pi-coding-agent --test interactive_event_bridge
cargo test -p pi-coding-agent --test interactive_mode
```

GREEN result: interactive product-event boundary tests passed, the existing event bridge tests passed, and all 41 interactive mode integration tests passed with unchanged UI behavior.
