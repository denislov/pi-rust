const AGENT_INVOCATION_FLOW: &str = include_str!("../src/coding_session/agent_invocation_flow.rs");
const AGENT_TEAM_FLOW: &str = include_str!("../src/coding_session/agent_team_flow.rs");
const BRANCH_SUMMARY_SERVICE: &str =
    include_str!("../src/coding_session/branch_summary_service.rs");
const FLOW_SERVICE: &str = include_str!("../src/coding_session/flow_service.rs");
const MANUAL_COMPACTION_SERVICE: &str =
    include_str!("../src/coding_session/manual_compaction_service.rs");
const PROMPT_CONTEXT: &str = include_str!("../src/coding_session/prompt.rs");
const PROMPT_EXECUTION: &str = include_str!("../src/coding_session/prompt_execution.rs");
const RPC_STATS: &str = include_str!("../src/protocol/rpc/stats.rs");
const INTERACTIVE_EVENT_BRIDGE: &str = include_str!("../src/interactive/event_bridge.rs");
const INTERACTIVE_LOOP: &str = include_str!("../src/interactive/loop.rs");
const SESSION_SERVICE: &str = include_str!("../src/coding_session/session_service.rs");
const PUBLIC_EVENT: &str = include_str!("../src/coding_session/public_event.rs");
const INTERNAL_EVENT: &str = include_str!("../src/coding_session/event.rs");
const PUBLIC_PROJECTION: &str = include_str!("../src/coding_session/public_projection.rs");
const PUBLIC_OPERATION: &str = include_str!("../src/coding_session/public_operation.rs");
const PRODUCT_EVENT_CONTRACT: &str = include_str!("../../../docs/product-event-contract.md");
const CRATE_ROOT: &str = include_str!("../src/lib.rs");
const PROTOCOL_EVENT_ADAPTER: &str = include_str!("../src/protocol/events.rs");
const PROTOCOL_EVENT_TESTS: &str = include_str!("protocol_events.rs");
const INTERACTIVE_EVENT_TESTS: &str = include_str!("interactive_event_bridge.rs");

#[test]
fn typed_public_event_boundary_is_fail_closed() {
    let documented_events = region(
        PRODUCT_EVENT_CONTRACT,
        "<!-- product-event-inventory:start -->",
        "<!-- product-event-inventory:end -->",
    );
    assert_eq!(
        documented_events
            .lines()
            .filter(|line| line.starts_with("| `"))
            .count(),
        46,
        "the authoritative product-event inventory must contain 46 rows"
    );
    for forbidden in ["format!(\"{:?}\"", "format!(\"{:#?}\""] {
        assert!(
            !PUBLIC_EVENT.contains(forbidden) && !PUBLIC_PROJECTION.contains(forbidden),
            "public event identity must not be derived through Debug formatting: {forbidden}"
        );
    }

    for line in PUBLIC_EVENT.lines().filter(|line| line.contains("pub ")) {
        assert!(
            !line.contains("CodingAgentEvent"),
            "public event declaration leaks the compatibility event: {line}"
        );
    }

    let conversion = region(
        PUBLIC_EVENT,
        "impl From<&CodingAgentEvent> for CodingAgentProductEventKind",
        "#[cfg(test)]",
    );
    assert!(
        !conversion
            .lines()
            .any(|line| line.trim_start().starts_with("_ =>")),
        "compatibility-to-public conversion must remain exhaustive without a wildcard"
    );

    for family in [
        "Session",
        "Profile",
        "Agent",
        "Team",
        "Message",
        "Tool",
        "Runtime",
        "Delegation",
        "Workflow",
        "Diagnostic",
        "Capability",
    ] {
        assert!(
            PRODUCT_EVENT_CONTRACT.contains(&format!("| {family} |")),
            "product-event contract omits family {family}"
        );
    }
}

#[test]
fn full_event_inventory_is_source_fixture_and_document_complete() {
    let source = internal_event_variants(INTERNAL_EVENT);
    let fixture = fixture_event_variants(region(
        PUBLIC_EVENT,
        "// product-event-fixture:start",
        "// product-event-fixture:end",
    ));
    let expected = public_inventory_rows(region(
        PUBLIC_EVENT,
        "// product-event-inventory:start",
        "// product-event-inventory:end",
    ));
    let documented = documented_inventory_rows(region(
        PRODUCT_EVENT_CONTRACT,
        "<!-- product-event-inventory:start -->",
        "<!-- product-event-inventory:end -->",
    ));

    assert_eq!(
        source.len(),
        46,
        "internal event enum must contain 46 variants"
    );
    assert_eq!(fixture.len(), 46, "fixture must construct 46 variants");
    assert_eq!(expected.len(), 46, "test inventory must contain 46 rows");
    assert_eq!(
        documented.len(),
        46,
        "document inventory must contain 46 rows"
    );
    assert_eq!(source, fixture, "fixture drifted from CodingAgentEvent");
    assert_eq!(
        expected, documented,
        "documented product-event inventory drifted from the executable inventory"
    );
    let expected_variants: std::collections::BTreeSet<_> = expected
        .iter()
        .map(|(variant, _, _)| variant.clone())
        .collect();
    assert_eq!(
        source, expected_variants,
        "inventory omitted or added an internal variant"
    );
}

#[test]
fn operation_outcome_documentation_matches_public_enums_exactly() {
    let operations = enum_variants(PUBLIC_OPERATION, "CodingAgentOperation");
    let outcomes = enum_variants(PUBLIC_OPERATION, "CodingAgentOperationOutcome");
    let matrix = region(
        PRODUCT_EVENT_CONTRACT,
        "<!-- operation-outcome-matrix:start -->",
        "<!-- operation-outcome-matrix:end -->",
    );
    let rows: Vec<_> = matrix
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let columns: Vec<_> = line.split('|').map(str::trim).collect();
            (
                columns[1].trim_matches('`').to_owned(),
                columns[2].trim_matches('`').to_owned(),
            )
        })
        .collect();

    assert_eq!(
        rows.len(),
        operations.len(),
        "one matrix row is required per operation"
    );
    assert_eq!(
        rows.len(),
        outcomes.len(),
        "one matrix row is required per outcome"
    );
    let documented_operations: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|(operation, _)| operation.clone())
        .collect();
    let documented_outcomes: std::collections::BTreeSet<_> =
        rows.iter().map(|(_, outcome)| outcome.clone()).collect();
    assert_eq!(
        documented_operations.len(),
        rows.len(),
        "operation rows must be unique"
    );
    assert_eq!(
        documented_outcomes.len(),
        rows.len(),
        "outcome rows must be unique"
    );
    assert_eq!(documented_operations, operations.into_iter().collect());
    assert_eq!(documented_outcomes, outcomes.into_iter().collect());
}

fn region<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start = source
        .find(start)
        .unwrap_or_else(|| panic!("missing region start: {start}"));
    let rest = &source[start..];
    let end = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing region end: {end}"));
    &rest[..end]
}

fn enum_variants(source: &str, enum_name: &str) -> std::collections::BTreeSet<String> {
    let declaration = format!("pub enum {enum_name} {{");
    let start = source
        .find(&declaration)
        .unwrap_or_else(|| panic!("missing {enum_name}"));
    let body = &source[start + declaration.len()..];
    let mut depth = 1_i32;
    let mut variants = std::collections::BTreeSet::new();
    for line in body.lines() {
        if depth == 1 {
            let trimmed = line.trim();
            let candidate = trimmed
                .split(['(', ' ', '{', ','])
                .next()
                .unwrap_or_default();
            if candidate.chars().next().is_some_and(char::is_uppercase) {
                variants.insert(candidate.to_owned());
            }
        }
        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if depth == 0 {
            break;
        }
    }
    assert!(
        !variants.is_empty(),
        "{enum_name} inventory must not be empty"
    );
    variants
}

fn internal_event_variants(source: &str) -> std::collections::BTreeSet<String> {
    enum_variants(source, "CodingAgentEvent")
}

fn fixture_event_variants(source: &str) -> std::collections::BTreeSet<String> {
    let mut variants = std::collections::BTreeSet::new();
    for line in source.lines() {
        let mut rest = line;
        while let Some(start) = rest.find("CodingAgentEvent::") {
            rest = &rest[start + "CodingAgentEvent::".len()..];
            let name = rest
                .split(['{', '(', ',', ' ', '\n'])
                .next()
                .unwrap_or_default();
            if !name.is_empty() && name.chars().next().is_some_and(char::is_uppercase) {
                variants.insert(name.to_owned());
            }
        }
    }
    variants
}

fn public_inventory_rows(source: &str) -> std::collections::BTreeSet<(String, String, String)> {
    let mut strings = Vec::new();
    let mut families = Vec::new();
    for line in source.lines() {
        let quoted: Vec<_> = line.split('"').collect();
        if quoted.len() >= 3 {
            strings.push(quoted[1].to_owned());
        }
        if let Some(family) = line
            .split("CodingAgentProductEventFamily::")
            .nth(1)
            .and_then(|value| value.split([',', ' ', '\n']).next())
        {
            families.push(family.to_ascii_lowercase());
        }
    }
    assert_eq!(
        strings.len(),
        families.len() * 2,
        "malformed executable inventory"
    );
    families
        .into_iter()
        .enumerate()
        .map(|(index, family)| {
            (
                strings[index * 2].clone(),
                family,
                strings[index * 2 + 1].clone(),
            )
        })
        .collect()
}

fn documented_inventory_rows(source: &str) -> std::collections::BTreeSet<(String, String, String)> {
    source
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let columns: Vec<_> = line.split('|').map(str::trim).collect();
            assert_eq!(
                columns.len(),
                5,
                "malformed documented inventory row: {line}"
            );
            (
                columns[1].trim_matches('`').to_owned(),
                columns[2].trim_matches('`').to_owned(),
                columns[3].trim_matches('`').to_owned(),
            )
        })
        .collect()
}

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
    let owner_impl = PROMPT_EXECUTION
        .split("impl CodingAgentSession {")
        .nth(1)
        .expect("prompt execution owner impl should be present");
    let finalize_region = owner_impl
        .split("    fn finalize_prompt_transaction(")
        .nth(1)
        .expect("owner finalize_prompt_transaction should be present");

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
    let owner_helpers = PROMPT_EXECUTION
        .split("fn apply_finalized_session_write(")
        .nth(1)
        .expect("apply_finalized_session_write helper should be present");

    assert!(
        !owner_helpers.contains("PromptTurnOutcome::Success"),
        "CodingAgentSession owner should delegate prompt success session/leaf metadata updates to PromptTurnOutcome helpers"
    );
}

#[test]
fn prompt_inner_uses_outcome_helper_for_success_branching() {
    let prompt_inner = PROMPT_EXECUTION
        .split("async fn prompt_inner(")
        .nth(1)
        .expect("prompt_inner should be present")
        .split("    async fn execute_authorized_delegations(")
        .next()
        .expect("delegation execution should follow prompt_inner");

    assert!(
        !prompt_inner.contains("PromptTurnOutcome::Success"),
        "prompt_inner should ask PromptTurnOutcome helpers about success state instead of matching the success variant inline"
    );
}

#[test]
fn rpc_state_consumes_public_client_snapshot_boundary() {
    assert!(
        RPC_STATS.contains("client_connection") && RPC_STATS.contains("connection.state()"),
        "RPC state projection should consume the public client connection snapshot boundary"
    );
    assert!(
        !RPC_STATS.contains(".persistent_session_service("),
        "RPC state projection should not bypass the public snapshot by reading persistent session service directly"
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

#[test]
fn stable_facade_and_adapters_reject_raw_event_projection() {
    let stable_api = region(
        CRATE_ROOT,
        "pub mod api {",
        "#[cfg(any(test, feature = \"test-harness\", debug_assertions))]",
    );
    assert!(
        !stable_api.contains("CodingAgentEvent"),
        "the stable facade must not export the private raw admission event"
    );
    assert!(
        !PROTOCOL_EVENT_ADAPTER.contains("pub fn push(&mut self, event: &CodingAgentEvent)"),
        "the protocol adapter must not expose a public raw-event projection method"
    );
    assert!(
        !INTERACTIVE_EVENT_BRIDGE.contains("pub fn handle(&mut self, event: &CodingAgentEvent)"),
        "the interactive bridge must not expose a public raw-event projection method"
    );
    assert!(
        !PROTOCOL_EVENT_TESTS.contains("CodingAgentEvent")
            && !INTERACTIVE_EVENT_TESTS.contains("CodingAgentEvent"),
        "first-party adapter behavior tests must enter through typed product-event fixtures"
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
    let allowed = ["crates/pi-coding-agent/tests/event_boundary_guards.rs"];
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

#[test]
fn legacy_receiver_and_duplicate_broadcast_are_absent() {
    let session_source = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/mod.rs",
    ))
    .expect("read coding session owner");
    let connection_source = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/session_connection.rs",
    ))
    .expect("read coding session connection owner");
    let owner_source = format!("{session_source}\n{connection_source}");
    let event_service_source = std::fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/event_service.rs",
    ))
    .expect("read event service");

    let owner_forbidden = [
        "pub use event_service::CodingAgentEventReceiver",
        "pub fn subscribe(&self) -> CodingAgentEventReceiver",
        "use subscribe_product_events_public instead",
        "#[allow(deprecated)]\n    pub fn subscribe(",
    ];
    let event_service_forbidden = [
        "struct CodingAgentEventReceiver",
        "impl CodingAgentEventReceiver",
        "pub(crate) fn subscribe(&self)",
        "Sender<CodingAgentEvent>",
        ".sender\n            .send(",
        "use ProductEventReceiver instead",
        "#[allow(deprecated)]\nmod tests",
    ];

    for forbidden in owner_forbidden {
        assert!(
            !owner_source.contains(forbidden),
            "coding session owner reintroduced legacy receiver/subscription fragment: {forbidden}"
        );
    }
    for forbidden in event_service_forbidden {
        assert!(
            !event_service_source.contains(forbidden),
            "EventService reintroduced legacy receiver/duplicate broadcast fragment: {forbidden}"
        );
    }

    assert!(owner_source.contains("pub fn subscribe_product_events_public(&self)"));
    assert!(event_service_source.contains("broadcast::Sender<ProductEvent>"));
    assert!(event_service_source.contains("self.product_sender.send(product_event.clone())"));
    assert!(event_service_source.contains("retained_product_events.push_back(event)"));
}

#[test]
fn production_event_runtime_has_no_raw_compatibility_storage_or_transport() {
    let repo_root = workspace_path("");
    let scan_roots = [
        "crates/pi-coding-agent/src/coding_session",
        "crates/pi-coding-agent/src/protocol",
        "crates/pi-coding-agent/src/interactive",
        "crates/pi-coding-agent/src/lib.rs",
    ];
    let forbidden = [
        ["compatibility", "_event"].concat(),
        "CodingAgentEventReceiver".to_owned(),
        "Sender<CodingAgentEvent>".to_owned(),
        "Receiver<CodingAgentEvent>".to_owned(),
        "broadcast::channel::<CodingAgentEvent>".to_owned(),
        ["from_compat", "_event"].concat(),
    ];
    let mut violations = Vec::new();

    for root in scan_roots {
        collect_source_violations(
            &repo_root,
            &repo_root.join(root),
            &[],
            &mut violations,
            |line| forbidden.iter().any(|needle| line.contains(needle)),
        );
    }

    assert!(
        violations.is_empty(),
        "production event code reintroduced raw compatibility storage, accessors, receivers, broadcasts, or conversions:\n{}",
        violations.join("\n")
    );

    let event_source = std::fs::read_to_string(
        repo_root.join("crates/pi-coding-agent/src/coding_session/event.rs"),
    )
    .expect("read internal event source");
    assert!(event_source.contains("#[cfg(test)]\n    pub(crate) fn from_event_for_tests("));

    let event_service_source = std::fs::read_to_string(
        repo_root.join("crates/pi-coding-agent/src/coding_session/event_service.rs"),
    )
    .expect("read event service source");
    let emit = region(
        &event_service_source,
        "pub(crate) fn emit(&self, event: CodingAgentEvent) -> ProductEvent",
        "pub(crate) fn emit_agent_event",
    );
    assert_eq!(
        emit.matches("CodingAgentProductEventKind::from(&event)")
            .count(),
        1,
        "EventService::emit must convert each raw event exactly once"
    );
    assert!(emit.contains("ProductEvent::new("));
    assert!(
        !emit
            .lines()
            .any(|line| line.trim() == "let raw_event = event.clone();")
    );
}

#[test]
fn compatibility_deletion_does_not_add_path_scoped_deprecation_suppressions() {
    let repo_root = workspace_path("");
    let guarded_files = [
        "crates/pi-coding-agent/src/coding_session/event.rs",
        "crates/pi-coding-agent/src/coding_session/event_service.rs",
        "crates/pi-coding-agent/src/coding_session/mod.rs",
        "crates/pi-coding-agent/src/protocol/events.rs",
        "crates/pi-coding-agent/src/protocol/rpc/events.rs",
        "crates/pi-coding-agent/src/interactive/event_bridge.rs",
        "crates/pi-coding-agent/src/interactive/loop.rs",
    ];
    let mut violations = Vec::new();

    for relative in guarded_files {
        let source =
            std::fs::read_to_string(repo_root.join(relative)).expect("read guarded source");
        for (line_index, line) in source.lines().enumerate() {
            if line.contains("allow(deprecated)") {
                violations.push(format!("{relative}:{}: {}", line_index + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "compatibility deletion paths must not suppress deprecated APIs:\n{}",
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
