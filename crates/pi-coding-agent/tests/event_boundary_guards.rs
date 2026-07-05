const AGENT_INVOCATION_FLOW: &str = include_str!("../src/coding_session/agent_invocation_flow.rs");
const AGENT_TEAM_FLOW: &str = include_str!("../src/coding_session/agent_team_flow.rs");
const CODING_SESSION_OWNER: &str = include_str!("../src/coding_session/mod.rs");
const PROMPT_CONTEXT: &str = include_str!("../src/coding_session/prompt.rs");
const SESSION_SERVICE: &str = include_str!("../src/coding_session/session_service.rs");

#[test]
fn workflow_flows_emit_diagnostics_through_event_service_helpers() {
    for (name, source) in [
        ("agent_invocation_flow", AGENT_INVOCATION_FLOW),
        ("agent_team_flow", AGENT_TEAM_FLOW),
    ] {
        assert!(
            !source.contains("self.event_service.emit(CodingAgentEvent::Diagnostic"),
            "{name} constructs diagnostic events directly instead of using EventService::emit_diagnostic"
        );
    }
}

#[test]
fn prompt_context_records_completion_through_event_service_helper() {
    assert!(
        !PROMPT_CONTEXT.contains("self.coding_events.push(CodingAgentEvent::PromptCompleted"),
        "PromptTurnContext should build prompt-completed events through EventService helpers"
    );
}

#[test]
fn session_service_builds_session_write_events_through_event_service_helpers() {
    let production_source = SESSION_SERVICE
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("session_service source should be present");

    assert!(
        !production_source.contains("CodingAgentEvent::SessionWrite"),
        "SessionService should build session-write events through EventService helpers"
    );
}

#[test]
fn workflow_flows_emit_prompt_outcomes_through_event_service_helpers() {
    for (name, source) in [
        ("agent_invocation_flow", AGENT_INVOCATION_FLOW),
        ("agent_team_flow", AGENT_TEAM_FLOW),
    ] {
        for event_name in ["PromptCompleted", "PromptAborted", "PromptFailed"] {
            let needle = format!("self.event_service.emit(CodingAgentEvent::{event_name}");
            assert!(
                !source.contains(&needle),
                "{name} constructs {event_name} directly instead of using EventService prompt outcome helpers"
            );
        }
    }
}

#[test]
fn manual_compaction_prompt_outcomes_are_built_by_flow_boundary() {
    let compact_inner = CODING_SESSION_OWNER
        .split("    async fn compact_inner(")
        .nth(1)
        .expect("compact_inner should be present")
        .split("    fn reused_branch_summary_outcome")
        .next()
        .expect("branch summary boundary should follow compact_inner");

    for variant in ["PromptTurnOutcome::Success", "PromptTurnOutcome::Failed"] {
        assert!(
            !compact_inner.contains(variant),
            "compact_inner should delegate manual compaction outcome construction to the flow boundary instead of building {variant} inline"
        );
    }
}

#[test]
fn branch_summary_prompt_outcomes_are_built_by_flow_boundary() {
    let branch_summary_region = CODING_SESSION_OWNER
        .split("    fn reused_branch_summary_outcome(")
        .nth(1)
        .expect("reused_branch_summary_outcome should be present")
        .split("    fn apply_default_agent_profile")
        .next()
        .expect("apply_default_agent_profile should follow branch summary helpers");

    for variant in ["PromptTurnOutcome::Success", "PromptTurnOutcome::Failed"] {
        assert!(
            !branch_summary_region.contains(variant),
            "branch summary owner methods should delegate outcome construction to the branch summary flow boundary instead of building {variant} inline"
        );
    }
}

#[test]
fn owner_delegates_prompt_transaction_finalization_to_services() {
    let owner_impl = CODING_SESSION_OWNER
        .split("impl CodingAgentSession {")
        .nth(1)
        .expect("CodingAgentSession impl should be present");
    let finalize_region = owner_impl
        .split("    fn finalize_prompt_transaction(")
        .nth(1)
        .expect("owner finalize_prompt_transaction should be present")
        .split("    #[cfg(test)]")
        .next()
        .expect("test section should follow owner helpers");

    for variant in [
        "PromptTurnOutcome::Success",
        "PromptTurnOutcome::Aborted",
        "PromptTurnOutcome::Failed",
    ] {
        assert!(
            !finalize_region.contains(variant),
            "CodingAgentSession::finalize_prompt_transaction should delegate {variant} handling to session/transient services"
        );
    }
}

#[test]
fn owner_does_not_rebuild_prompt_success_session_write_metadata() {
    let owner_helpers = CODING_SESSION_OWNER
        .split("fn apply_finalized_session_write(")
        .nth(1)
        .expect("apply_finalized_session_write helper should be present")
        .split("#[cfg(test)]")
        .next()
        .expect("test section should follow owner helpers");

    assert!(
        !owner_helpers.contains("PromptTurnOutcome::Success"),
        "CodingAgentSession owner should delegate prompt success session/leaf metadata updates to PromptTurnOutcome helpers"
    );
}

#[test]
fn prompt_inner_uses_outcome_helper_for_success_branching() {
    let prompt_inner = CODING_SESSION_OWNER
        .split("    async fn prompt_inner(")
        .nth(1)
        .expect("prompt_inner should be present")
        .split("    async fn invoke_agent_inner(")
        .next()
        .expect("invoke_agent_inner should follow prompt_inner");

    assert!(
        !prompt_inner.contains("PromptTurnOutcome::Success"),
        "prompt_inner should ask PromptTurnOutcome helpers about success state instead of matching the success variant inline"
    );
}
