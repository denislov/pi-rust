const AGENT_INVOCATION_FLOW: &str = include_str!("../src/coding_session/agent_invocation_flow.rs");
const AGENT_TEAM_FLOW: &str = include_str!("../src/coding_session/agent_team_flow.rs");
const BRANCH_SUMMARY_SERVICE: &str =
    include_str!("../src/coding_session/branch_summary_service.rs");
const CODING_SESSION_OWNER: &str = include_str!("../src/coding_session/mod.rs");
const FLOW_SERVICE: &str = include_str!("../src/coding_session/flow_service.rs");
const MANUAL_COMPACTION_SERVICE: &str =
    include_str!("../src/coding_session/manual_compaction_service.rs");
const PROMPT_CONTEXT: &str = include_str!("../src/coding_session/prompt.rs");
const RPC_STATS: &str = include_str!("../src/protocol/rpc/stats.rs");
const INTERACTIVE_EVENT_BRIDGE: &str = include_str!("../src/interactive/event_bridge.rs");
const INTERACTIVE_LOOP: &str = include_str!("../src/interactive/loop.rs");
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

#[test]
fn rpc_state_consumes_ui_snapshot_boundary() {
    assert!(
        RPC_STATS.contains(".ui_snapshot("),
        "RPC state projection should consume the UiSnapshot boundary"
    );
    assert!(
        !RPC_STATS.contains(".persistent_session_service("),
        "RPC state projection should not bypass UiSnapshot by reading persistent session service directly"
    );
}

#[test]
fn interactive_projection_consumes_product_events() {
    assert!(
        INTERACTIVE_EVENT_BRIDGE.contains("UiProjection"),
        "interactive projection should use UiProjection"
    );
    assert!(
        INTERACTIVE_EVENT_BRIDGE.contains("push_product_event"),
        "interactive projection should consume product events through UiProjection"
    );
    assert!(
        INTERACTIVE_LOOP.contains("ui_projection: &mut UiProjection"),
        "interactive prompt-task event application should receive a UiProjection instead of projecting directly through CodingEventBridge"
    );
    assert!(
        INTERACTIVE_LOOP.contains("UiProjection::from_snapshot"),
        "interactive prompt-task event application should reset projection state from UiSnapshot"
    );
    assert!(
        INTERACTIVE_LOOP.contains("ui_projection.apply_product_event"),
        "interactive prompt-task event application should consume ProductEvent through UiProjection"
    );
}

fn workspace_path(relative: &str) -> std::path::PathBuf {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crate should live under crates/pi-coding-agent")
        .to_path_buf();
    repo_root.join(relative)
}

#[test]
fn first_party_code_does_not_consume_compatibility_event_subscription() {
    let scan_roots = [
        "crates/pi-coding-agent/src/protocol",
        "crates/pi-coding-agent/src/interactive",
        "crates/pi-coding-agent/tests",
    ];
    let repo_root = workspace_path("");
    let allowed = [
        "crates/pi-coding-agent/tests/public_api.rs",
        "crates/pi-coding-agent/tests/event_boundary_guards.rs",
    ];
    let mut violations = Vec::new();

    for root in scan_roots {
        collect_source_violations(
            &repo_root,
            &repo_root.join(root),
            &allowed,
            &mut violations,
            |line| line.contains(".subscribe()") || line.contains("CodingAgentEventReceiver"),
        );
    }

    assert!(
        violations.is_empty(),
        "first-party code should consume ProductEvent or public product-event facades instead of compatibility CodingAgentEventReceiver:\n{}",
        violations.join("\n")
    );
}

fn collect_source_violations(
    repo_root: &std::path::Path,
    path: &std::path::Path,
    allowed_files: &[&str],
    violations: &mut Vec<String>,
    is_violation: impl Copy + Fn(&str) -> bool,
) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .expect("read source directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read source entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_source_violations(
                repo_root,
                &entry.path(),
                allowed_files,
                violations,
                is_violation,
            );
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }
    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    if allowed_files.contains(&relative.as_str()) {
        return;
    }
    let content = std::fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        if is_violation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

#[test]
fn rpc_protocol_exposes_optional_version_negotiation_state() {
    let types_rs = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/protocol/types.rs",
    ))
    .expect("read protocol types");
    let commands_rs = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/protocol/rpc/commands.rs",
    ))
    .expect("read rpc commands");
    let state_rs = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/protocol/rpc/state.rs",
    ))
    .expect("read rpc state");
    let stats_rs = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/protocol/rpc/stats.rs",
    ))
    .expect("read rpc stats");

    assert!(types_rs.contains("Hello {"));
    assert!(commands_rs.contains("RPC_PROTOCOL_VERSION.is_compatible_with"));
    assert!(state_rs.contains("negotiated_protocol"));
    assert!(stats_rs.contains("negotiated_protocol"));
}

#[test]
fn startup_recovery_stays_session_service_owned() {
    let session_service_rs = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/session_service.rs",
    ))
    .expect("read session service source");
    let rpc_sources = [
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/commands.rs"),
        workspace_path("crates/pi-coding-agent/src/protocol/rpc/prompt.rs"),
        workspace_path("crates/pi-coding-agent/src/interactive/event_bridge.rs"),
    ];

    assert!(session_service_rs.contains("apply_startup_recovery"));
    assert!(session_service_rs.contains("take_startup_recovery_markers"));
    for source in rpc_sources {
        let text = std::fs::read_to_string(&source).expect("read adapter source");
        assert!(
            !text.contains("SessionEventData::OperationRecovered {"),
            "adapters must project recovery events but not write recovery session markers: {}",
            source.display()
        );
    }
}
