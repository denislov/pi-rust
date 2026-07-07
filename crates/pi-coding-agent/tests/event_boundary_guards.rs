const AGENT_INVOCATION_FLOW: &str = include_str!("../src/coding_session/agent_invocation_flow.rs");
const AGENT_TEAM_FLOW: &str = include_str!("../src/coding_session/agent_team_flow.rs");
const BRANCH_SUMMARY_SERVICE: &str =
    include_str!("../src/coding_session/branch_summary_service.rs");
const CODING_SESSION_OWNER: &str = include_str!("../src/coding_session/mod.rs");
const FLOW_SERVICE: &str = include_str!("../src/coding_session/flow_service.rs");
const MANUAL_COMPACTION_SERVICE: &str =
    include_str!("../src/coding_session/manual_compaction_service.rs");
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
fn nested_workflows_use_explicit_subflow_runners() {
    for method in [
        "run_prompt_subflow_for_agent_invocation",
        "run_agent_invocation_subflow",
        "run_agent_team_subflow",
    ] {
        assert!(
            FLOW_SERVICE.contains(method),
            "FlowService should expose explicit nested workflow subflow runner `{method}`"
        );
    }

    for (name, source) in [
        ("agent_invocation_flow", AGENT_INVOCATION_FLOW),
        ("agent_team_flow", AGENT_TEAM_FLOW),
    ] {
        for needle in [
            "PromptTurnFlow::new()?.run",
            "AgentInvocationFlow::new()?.run",
            "AgentTeamFlow::new()?.run",
        ] {
            assert!(
                !source.contains(needle),
                "{name} should route nested workflow execution through FlowService subflow runners instead of `{needle}`"
            );
        }
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
    assert!(
        MANUAL_COMPACTION_SERVICE.contains("manual_compaction_success_outcome")
            && MANUAL_COMPACTION_SERVICE.contains("manual_compaction_failed_outcome"),
        "ManualCompactionService should delegate manual compaction outcome construction to flow-boundary helpers"
    );

    for variant in ["PromptTurnOutcome::Success", "PromptTurnOutcome::Failed"] {
        assert!(
            !MANUAL_COMPACTION_SERVICE.contains(variant),
            "ManualCompactionService should delegate manual compaction outcome construction to the flow boundary instead of building {variant} inline"
        );
    }
}

#[test]
fn branch_summary_prompt_outcomes_are_built_by_flow_boundary() {
    assert!(
        BRANCH_SUMMARY_SERVICE.contains("branch_summary_success_outcome")
            && BRANCH_SUMMARY_SERVICE.contains("branch_summary_failed_outcome"),
        "BranchSummaryService should delegate branch-summary outcome construction to flow-boundary helpers"
    );

    for variant in ["PromptTurnOutcome::Success", "PromptTurnOutcome::Failed"] {
        assert!(
            !BRANCH_SUMMARY_SERVICE.contains(variant),
            "BranchSummaryService should delegate outcome construction to the branch summary flow boundary instead of building {variant} inline"
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
