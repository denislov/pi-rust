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

- [ ] **Step 1: Write failing family classification tests**

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
        let completed = CodingAgentEvent::PromptCompleted {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
        }
        .classification();
        assert_eq!(completed.operation_id, Some("op_prompt"));
        assert_eq!(
            completed.terminal_status,
            Some(ProductEventTerminalStatus::Completed)
        );

        let failed = CodingAgentEvent::SelfHealingEditFailed {
            operation_id: "op_edit".into(),
            path: "src/lib.rs".into(),
            error: CodingSessionError::SelfHealingEditFailed {
                message: "check failed".into(),
                diagnostics: Vec::new(),
                check_output: None,
            },
        }
        .classification();
        assert_eq!(failed.operation_id, Some("op_edit"));
        assert_eq!(
            failed.terminal_status,
            Some(ProductEventTerminalStatus::Failed)
        );

        let aborted = CodingAgentEvent::AgentInvocationAborted {
            operation_id: "op_agent".into(),
            child_operation_id: "op_child".into(),
            profile_id: profile_id("agent-main"),
            reason: "cancelled".into(),
        }
        .classification();
        assert_eq!(aborted.operation_id, Some("op_agent"));
        assert_eq!(
            aborted.terminal_status,
            Some(ProductEventTerminalStatus::Aborted)
        );

        let progress = CodingAgentEvent::AssistantMessageDelta {
            operation_id: "op_prompt".into(),
            turn_id: "turn_1".into(),
            message_id: Some("msg_1".into()),
            text: "hello".into(),
        }
        .classification();
        assert_eq!(progress.operation_id, Some("op_prompt"));
        assert_eq!(progress.terminal_status, None);

        let uncorrelated = CodingAgentEvent::CapabilityChanged.classification();
        assert_eq!(uncorrelated.operation_id, None);
        assert_eq!(uncorrelated.terminal_status, None);
    }
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p pi-coding-agent coding_agent_events_report_internal_product_families --lib
```

Expected: compile fails because `classification`, `ProductEventFamily`, `ProductEventTerminalStatus`, and `ProductEventClassification` do not exist.

- [ ] **Step 3: Add the minimal internal classification model**

Add these crate-internal types near the top of `crates/pi-coding-agent/src/coding_session/event.rs`, after `CodingAgentEvent` imports and before the enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProductEventClassification<'event> {
    pub(crate) family: ProductEventFamily,
    pub(crate) operation_id: Option<&'event str>,
    pub(crate) terminal_status: Option<ProductEventTerminalStatus>,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductEventTerminalStatus {
    Completed,
    Failed,
    Aborted,
}
```

Add an `impl CodingAgentEvent` block that returns a `ProductEventClassification<'_>` for every current enum variant. The match must be exhaustive: adding a future `CodingAgentEvent` variant should require choosing a family and terminal status before it compiles.

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
cargo test -p pi-coding-agent coding_agent_events_report_internal_product_families --lib
cargo test -p pi-coding-agent coding_agent_events_report_operation_correlation_and_terminal_status --lib
```

Expected: both tests pass.

### Task 2: Verify Adapter Compatibility For The Classification Slice

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/superpowers/plans/2026-07-07-product-event-family-convergence-plan.md`

- [ ] **Step 1: Run existing protocol adapter coverage**

Run:

```bash
cargo test -p pi-coding-agent protocol_events
cargo test -p pi-coding-agent protocol_jsonl
cargo test -p pi-coding-agent rpc_mode
cargo test -p pi-coding-agent interactive_event_bridge
cargo test -p pi-coding-agent coding_session_public_api_symbols_are_importable
```

Expected: all tests pass, proving the internal classification did not alter adapter-visible protocol behavior.

- [ ] **Step 2: Run operation/event focused checks**

Run:

```bash
cargo test -p pi-coding-agent event_service --lib
cargo test -p pi-coding-agent operation --lib
cargo check -p pi-coding-agent
cargo fmt --check
git diff --check
git status --short
```

Expected: all checks pass and the worktree contains only the classification slice, TODO update, and this plan update.

- [ ] **Step 3: Commit the classification slice**

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

- [ ] **Step 1: Record the next Stage 2 boundary after classification lands**

After Task 1 and Task 2 are committed, update `docs/TODO.md` to state that Stage 2 has an internal family/status classifier and that the next cut is a non-public `ProductEvent` wrapper or EventService publication boundary.

- [ ] **Step 2: Keep wrapper implementation out of this commit**

Do not introduce `ProductEvent` publication or adapter migration in the classification commit. The first Stage 2 commit must remain behavior-preserving and easy to revert.
