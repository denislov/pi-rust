use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionMethod {
    name: String,
    visibility: &'static str,
    test_only: bool,
    attributes: Vec<String>,
    body: String,
    file: String,
    line: usize,
    end_line: usize,
}

#[derive(Debug, Clone, Copy)]
struct MethodExpectation {
    name: &'static str,
    group: &'static str,
    visibility: &'static str,
    test_only: bool,
}

#[test]
fn session_store_failure_controls_remain_test_only() {
    let scan = SourceScan::new();
    let store_path = scan
        .crate_root
        .join("src/coding_session/session_log/store.rs");
    let source = fs::read_to_string(&store_path).expect("read session store source");
    let sanitized = sanitize_rust_source(&source);

    for signature in [
        "failures: Arc<Mutex<StoreFailureState>>",
        "pub(crate) enum StoreFailurePoint",
        "struct StoreFailureState",
        "pub(crate) fn fail_after(",
        "fn fail_if_injected(",
    ] {
        assert_eq!(
            sanitized.matches(signature).count(),
            1,
            "session store test control must exist exactly once: {signature}"
        );
        assert_direct_cfg_test(&sanitized, signature);
    }

    for point in [
        "CreateBlobs",
        "CreateIndex",
        "WriteManifest",
        "CreateEventLog",
        "AppendEvents",
        "UpdateManifest",
        "RemoveSession",
    ] {
        let call = format!("self.fail_if_injected(StoreFailurePoint::{point})?");
        assert_eq!(
            sanitized.matches(&call).count(),
            1,
            "expected exactly one directly gated failure call for {point}"
        );
        assert_direct_cfg_test(&sanitized, &call);
    }

    let session_source = fs::read_to_string(scan.crate_root.join("src/coding_session/mod.rs"))
        .expect("read coding session source");
    let session_sanitized = sanitize_rust_source(&session_source);
    for signature in [
        "pub(crate) fn arm_append_events_failure_for_tests(",
        "pub(crate) fn arm_update_manifest_failure_for_tests(",
        "pub(crate) fn queue_pending_delegation_for_tests(",
    ] {
        assert_eq!(
            session_sanitized.matches(signature).count(),
            1,
            "owner-local test bridge must exist exactly once: {signature}"
        );
        assert_direct_cfg_test(&session_sanitized, signature);
    }

    let mut violations = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src/coding_session")) {
        let relative = relative_path(&scan.repo_root, &path);
        let source = fs::read_to_string(&path).expect("read coding-session source");
        let sanitized = sanitize_rust_source(&source);
        for (index, line) in sanitized.lines().enumerate() {
            let trimmed = line.trim();
            let fault_name = trimmed.contains("fail_after")
                || trimmed.contains("StoreFailurePoint")
                || trimmed.contains("StoreFailureState")
                || ((trimmed.contains("inject") || trimmed.contains("Injection"))
                    && (trimmed.contains("fail")
                        || trimmed.contains("failure")
                        || trimmed.contains("fault")))
                || trimmed.contains("FailureHook")
                || trimmed.contains("FaultPoint");
            if !fault_name || path == store_path {
                continue;
            }
            if !line_is_cfg_test_gated(&sanitized, index) {
                violations.push(format!("{}:{}: {}", relative, index + 1, trimmed));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "session-store failure controls must remain inside #[cfg(test)] items/modules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn final_receiver_aware_compatibility_absence_and_retained_api_guard() {
    let scan = SourceScan::new();
    let mut methods = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src/coding_session")) {
        collect_coding_agent_session_methods(&scan.repo_root, &path, &mut methods);
    }

    let mut expected = Vec::new();
    add_expectations(
        &mut expected,
        "canonical dispatcher",
        "pub",
        false,
        &["run"],
    );
    add_expectations(
        &mut expected,
        "retained owner-private custom-option path",
        "pub(crate)",
        false,
        &["load_plugins"],
    );
    let absent = [
        "invoke_agent",
        "invoke_team",
        "export_current",
        "export_current_html",
        "set_default_agent_profile_id",
        "prompt",
        "compact",
        "self_healing_edit",
        "self_healing_edit_with_options",
        "reload_plugins",
        "run_plugin_command",
        "approve_delegation_confirmation",
        "reject_delegation_confirmation",
        "fork_current_session",
        "summarize_branch",
        "summarize_branch_for_navigation",
        "subscribe",
    ];
    add_expectations(
        &mut expected,
        "retained lifecycle/query/event/control helper",
        "pub",
        false,
        &[
            "create",
            "open",
            "open_or_create",
            "non_persistent",
            "list",
            "export_session_html",
            "subscribe_product_events_public",
            "runtime_shutdown_handle",
            "shutdown",
            "snapshot",
            "connect",
            "capabilities",
            "view",
            "agent_profiles",
            "team_profiles",
            "profile_diagnostics",
            "pending_delegation_confirmations",
        ],
    );
    add_expectations(
        &mut expected,
        "retained lifecycle/query/event/control helper",
        "pub(crate)",
        false,
        &[
            "hydrate",
            "tree_view",
            "clone_session",
            "fork_session",
            "hydrate_current",
            "subscribe_product_events",
            "product_event_replay_handle",
            "compact_cancellation_handle",
            "ui_snapshot",
            "connect_client",
            "product_events_after",
            "prompt_control_handle",
            "plugin_commands",
            "plugin_ui_actions",
            "plugin_ui_dialogs",
            "plugin_keybindings",
        ],
    );
    add_expectations(
        &mut expected,
        "test-only helper",
        "pub(crate)",
        true,
        &[
            "non_persistent_with_event_capacity_for_tests",
            "non_persistent_with_event_capacities_for_tests",
            "emit_product_event_for_tests",
            "arm_append_events_failure_for_tests",
            "arm_update_manifest_failure_for_tests",
            "queue_pending_delegation_for_tests",
        ],
    );

    let mut violations = Vec::new();
    for name in absent {
        let definitions = methods.iter().filter(|method| method.name == name).count();
        if definitions != 0 {
            violations.push(format!(
                "deleted compatibility method `{name}` must have no CodingAgentSession definition, found {definitions}"
            ));
        }
    }
    violations.extend(absent_receiver_calls(&scan, &absent));
    violations.extend(local_deprecation_suppression_violations(&scan, &absent));
    violations.extend(load_plugins_owner_call_violations(&scan));
    for expectation in &expected {
        let definitions = methods
            .iter()
            .filter(|method| method.name == expectation.name)
            .collect::<Vec<_>>();
        if definitions.len() != 1 {
            violations.push(format!(
                "{} `{}` expected exactly once, found {}: {}",
                expectation.group,
                expectation.name,
                definitions.len(),
                format_method_locations(&definitions)
            ));
            continue;
        }
        let method = definitions[0];
        if method.visibility != expectation.visibility || method.test_only != expectation.test_only
        {
            violations.push(format!(
                "{} `{}` has visibility/test gate {}/{}, expected {}/{} at {}:{}",
                expectation.group,
                expectation.name,
                method.visibility,
                method.test_only,
                expectation.visibility,
                expectation.test_only,
                method.file,
                method.line
            ));
        }
    }
    for method in &methods {
        let groups = expected
            .iter()
            .filter(|expectation| expectation.name == method.name)
            .map(|expectation| expectation.group)
            .collect::<Vec<_>>();
        if groups.len() != 1 {
            let diagnostic_context = unexpected_method_context(method);
            violations.push(format!(
                "method `{}` belongs to {} allowed groups ({:?}) at {}:{}-{}{}",
                method.name,
                groups.len(),
                groups,
                method.file,
                method.line,
                method.end_line,
                diagnostic_context,
            ));
        }
    }
    violations.extend(alternate_facade_violations(&scan));

    let lib = sanitize_rust_source(
        &fs::read_to_string(scan.crate_root.join("src/lib.rs")).expect("read crate lib source"),
    );
    assert_eq!(
        lib.matches("pub mod api").count(),
        1,
        "lib.rs::api must remain the sole stable facade"
    );
    assert!(
        violations.is_empty(),
        "CodingAgentSession public/pub(crate) method ledger changed:\n{}",
        violations.join("\n")
    );
}

fn absent_receiver_calls(scan: &SourceScan, names: &[&str]) -> Vec<String> {
    let mut paths = rust_files_under(&scan.crate_root.join("src"));
    paths.extend(rust_files_under(&scan.crate_root.join("tests")));
    let mut violations = Vec::new();
    for path in paths {
        let relative = relative_path(&scan.repo_root, &path);
        if relative == "crates/pi-coding-agent/tests/event_boundary_guards.rs" {
            continue;
        }
        let source = sanitize_rust_source(&fs::read_to_string(&path).expect("read Rust source"));
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            for name in names {
                let pattern = format!(".{name}(");
                if line.contains(&pattern) {
                    if *name == "subscribe" && line.contains("lifecycle_sender") {
                        continue;
                    }
                    if (*name == "prompt" && line.contains("agent.prompt("))
                        || (*name == "set_default_agent_profile_id"
                            && (line.contains("session_service.set_default_agent_profile_id(")
                                || line.contains("root.set_default_agent_profile_id(")
                                || line.contains("self.set_default_agent_profile_id(")))
                    {
                        continue;
                    }
                    violations.push(format!(
                        "deleted G1 receiver call `{name}` remains at {relative}:{}: {}",
                        index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    violations
}

fn local_deprecation_suppression_violations(scan: &SourceScan, names: &[&str]) -> Vec<String> {
    let mut paths = rust_files_under(&scan.crate_root.join("src"));
    paths.extend(rust_files_under(&scan.crate_root.join("tests")));
    let mut violations = Vec::new();
    for path in paths {
        if relative_path(&scan.repo_root, &path)
            == "crates/pi-coding-agent/tests/event_boundary_guards.rs"
        {
            continue;
        }
        let source = sanitize_rust_source(&fs::read_to_string(&path).expect("read Rust source"));
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if !line.contains("#[allow(deprecated)]") {
                continue;
            }
            let window = lines[index..usize::min(index + 12, lines.len())].join("\n");
            if names.iter().any(|name| window.contains(name)) {
                violations.push(format!(
                    "local deprecated suppression remains near deleted compatibility method at {}:{}",
                    relative_path(&scan.repo_root, &path),
                    index + 1
                ));
            }
        }
    }
    violations
}

fn load_plugins_owner_call_violations(scan: &SourceScan) -> Vec<String> {
    let owner_path = scan.crate_root.join("src/coding_session/mod.rs");
    let mut paths = rust_files_under(&scan.crate_root.join("src"));
    paths.extend(rust_files_under(&scan.crate_root.join("tests")));
    let mut violations = Vec::new();
    let mut calls = 0;

    for path in paths {
        let relative = relative_path(&scan.repo_root, &path);
        let raw_source = fs::read_to_string(&path).expect("read Rust source");
        let source = sanitize_rust_source(&raw_source);
        let raw_lines = raw_source.lines().collect::<Vec<_>>();
        let lines = source.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if !line.contains(".load_plugins(") {
                continue;
            }
            calls += 1;
            if path != owner_path {
                violations.push(format!(
                    "load_plugins custom-option call escaped coding_session owner tests at {relative}:{}",
                    index + 1
                ));
                continue;
            }
            if !line_is_cfg_test_gated(&source, index) {
                violations.push(format!(
                    "load_plugins custom-option call is not test-gated at {relative}:{}",
                    index + 1
                ));
            }
            let prior = raw_lines[..index].iter().rev().take(4).collect::<Vec<_>>();
            if !prior.iter().any(|candidate| candidate.contains("D-03")) {
                violations.push(format!(
                    "load_plugins owner exception lacks D-03 insufficiency justification at {relative}:{}",
                    index + 1
                ));
            }
        }
    }

    if calls != 4 {
        violations.push(format!(
            "load_plugins must have exactly four owner-test calls, found {calls}"
        ));
    }
    violations
}

#[test]
fn product_sources_do_not_register_global_provider_runtime_outside_compat_boundary() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    collect_source_violations(
        scan.repo_root(),
        &scan.crate_root.join("src"),
        &[],
        &mut violations,
        |line| {
            line.contains("register_builtin_providers_for_global_runtime(")
                || line.contains("pi_ai::providers::register_builtins()")
        },
    );

    assert!(
        violations.is_empty(),
        "product source must not register the global provider runtime outside the explicit compatibility boundary; normal product execution uses scoped AiClient runtime paths:\n{}",
        violations.join("\n")
    );
}

#[test]
fn adapters_do_not_construct_or_run_low_level_agents() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    for relative_root in ["src/interactive", "src/protocol", "src/print_mode.rs"] {
        collect_source_violations(
            scan.repo_root(),
            &scan.crate_root.join(relative_root),
            &[],
            &mut violations,
            |line| {
                line.contains("Agent::new(")
                    || line.contains("Agent::with_messages(")
                    || line.contains("use pi_agent_core::api::Agent;")
                    || line.contains("use pi_agent_core::api::{Agent,")
                    || line.contains("use pi_agent_core::api::{ Agent,")
            },
        );
    }

    assert!(
        violations.is_empty(),
        "adapters should route product execution through CodingAgentSession instead of low-level Agent construction or execution:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_json_and_print_use_canonical_operations() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    // JSON/print adapter files must submit Prompt operations through
    // CodingAgentSession::run instead of deprecated broad workflow methods, and
    // must not suppress deprecation warnings in production source. Test-only
    // allowances inside #[cfg(test)] modules are preserved.
    let adapter_files = ["src/protocol/json_mode.rs", "src/print_mode.rs"];
    let deprecated_workflow_methods = [
        "prompt",
        "compact",
        "self_healing_edit_with_options",
        "invoke_agent",
        "invoke_team",
        "summarize_branch",
        "export_current",
        "export_current_html",
    ];

    for relative_path in adapter_files {
        let path = scan.crate_root.join(relative_path);
        let source =
            fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {relative_path}: {err}"));
        let sanitized = sanitize_rust_source(&source);
        for (index, line) in sanitized.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || line_is_cfg_test_gated(&sanitized, index) {
                continue;
            }
            if trimmed.contains("#[allow(deprecated)]") {
                violations.push(format!(
                    "{relative_path}:{}: production adapter suppresses deprecation: {}",
                    index + 1,
                    trimmed
                ));
            }
            for method in deprecated_workflow_methods {
                let pattern = format!(".{method}(");
                if trimmed.contains(&pattern) {
                    violations.push(format!(
                        "{relative_path}:{}: production adapter calls deprecated broad workflow method `{method}` instead of CodingAgentSession::run: {}",
                        index + 1,
                        trimmed
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "JSON/print production adapters must route operations through CodingAgentSession::run and must not suppress deprecated broad workflow calls:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_rpc_uses_canonical_operations() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    // RPC production source must submit operations through
    // CodingAgentSession::run instead of replaced broad workflow methods
    // (both deprecated and non-deprecated), and must not suppress
    // deprecation warnings in production source. Test-only allowances
    // inside #[cfg(test)] modules are preserved.
    let replaced_workflow_methods = [
        // Deprecated broad workflow methods
        "prompt",
        "compact",
        "self_healing_edit",
        "self_healing_edit_with_options",
        "invoke_agent",
        "invoke_team",
        "summarize_branch",
        "export_current",
        "export_current_html",
        // Non-deprecated methods replaced by canonical operations
        "approve_delegation_confirmation",
        "reject_delegation_confirmation",
        "set_default_agent_profile_id",
        "reload_plugins",
        "run_plugin_command",
    ];

    for path in rust_files_under(&scan.crate_root.join("src/protocol/rpc")) {
        let relative = relative_path(&scan.repo_root, &path);
        let source =
            fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {relative}: {err}"));
        let sanitized = sanitize_rust_source(&source);
        for (index, line) in sanitized.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || line_is_cfg_test_gated(&sanitized, index) {
                continue;
            }
            if trimmed.contains("#[allow(deprecated)]") {
                violations.push(format!(
                    "{relative}:{}: production RPC source suppresses deprecation: {}",
                    index + 1,
                    trimmed
                ));
            }
            for method in replaced_workflow_methods {
                let pattern = format!(".{method}(");
                if trimmed.contains(&pattern) {
                    violations.push(format!(
                        "{relative}:{}: production RPC source calls replaced workflow method `{method}` instead of CodingAgentSession::run: {}",
                        index + 1,
                        trimmed
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "RPC production source must route operations through CodingAgentSession::run and must not call replaced broad workflow methods or suppress deprecation:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_rpc_projects_the_public_client_connection_without_authority_mirrors() {
    let scan = SourceScan::new();
    let state_path = scan.crate_root.join("src/protocol/rpc/state.rs");
    let prompt_path = scan.crate_root.join("src/protocol/rpc/prompt.rs");
    let state = fs::read_to_string(&state_path).expect("read RPC state");
    let prompt = fs::read_to_string(&prompt_path).expect("read RPC prompt");
    let state_production = state.split("#[cfg(").next().unwrap();
    let prompt_production = prompt.split("#[cfg(").next().unwrap();

    assert!(state_production.contains("client_connection"));
    assert!(state_production.contains("CodingAgentClientConnection"));
    assert!(prompt_production.contains("connection.reconnect_from_cursor("));
    assert!(prompt_production.contains("connection.acknowledge("));
    assert!(prompt_production.contains("connection.prepare_submission("));
    assert!(prompt_production.contains("session.run("));

    for prohibited in [
        "client_drafts:",
        "submitted_operation:",
        "ProductEventReplayHandle",
        "PromptControlHandle",
        "replayed_through_sequence",
        "product_event_replay:",
    ] {
        assert!(
            !state_production.contains(prohibited) && !prompt_production.contains(prohibited),
            "RPC must not reintroduce client authority mirror `{prohibited}`"
        );
    }
}

#[test]
fn production_interactive_uses_canonical_operations() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    // Interactive production source must submit operations through
    // CodingAgentSession::run instead of replaced broad workflow methods
    // (both deprecated and non-deprecated), and must not suppress
    // deprecation warnings in production source. The legitimate local
    // InteractiveRoot::set_default_agent_profile_id projection setter is
    // explicitly allowed. Test-only allowances inside #[cfg(test)] modules
    // are preserved.
    let replaced_workflow_methods = [
        // Deprecated broad workflow methods
        "prompt",
        "compact",
        "self_healing_edit",
        "self_healing_edit_with_options",
        "invoke_agent",
        "invoke_team",
        "summarize_branch",
        "export_current",
        "export_current_html",
        // Non-deprecated methods replaced by canonical operations
        "approve_delegation_confirmation",
        "reject_delegation_confirmation",
        "reload_plugins",
        "run_plugin_command",
        "fork_current_session",
        "summarize_branch_for_navigation",
    ];

    for path in rust_files_under(&scan.crate_root.join("src/interactive")) {
        let relative = relative_path(&scan.repo_root, &path);
        let source =
            fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {relative}: {err}"));
        let sanitized = sanitize_rust_source(&source);
        for (index, line) in sanitized.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || line_is_cfg_test_gated(&sanitized, index) {
                continue;
            }
            if trimmed.contains("#[allow(deprecated)]") {
                violations.push(format!(
                    "{relative}:{}: production interactive source suppresses deprecation: {}",
                    index + 1,
                    trimmed
                ));
            }
            for method in replaced_workflow_methods {
                let pattern = format!(".{method}(");
                if trimmed.contains(&pattern) {
                    violations.push(format!(
                        "{relative}:{}: production interactive source calls replaced workflow method `{method}` instead of CodingAgentSession::run: {}",
                        index + 1,
                        trimmed
                    ));
                }
            }
            // set_default_agent_profile_id is both a legitimate InteractiveRoot
            // projection setter and a replaced CodingAgentSession method. Allow
            // root.set_default_agent_profile_id( and self.set_default_agent_profile_id(
            // (the root's own internal call); reject any other receiver.
            if trimmed.contains(".set_default_agent_profile_id(")
                && !trimmed.contains("root.set_default_agent_profile_id(")
                && !trimmed.contains("self.set_default_agent_profile_id(")
            {
                violations.push(format!(
                    "{relative}:{}: production interactive source calls replaced session method `set_default_agent_profile_id` on a non-root receiver instead of CodingAgentSession::run(SetDefaultAgentProfile): {}",
                    index + 1,
                    trimmed
                ));
            }
            // Reject private runtime contract imports from the coding_session
            // module. Migrated adapters must import operation types through
            // crate::api. Check the import prefix and private type names
            // separately to avoid false matches.
            let coding_session_prefix = ["crate::coding_", "session"].concat();
            if trimmed.contains("use ")
                && trimmed.contains(&coding_session_prefix)
                && trimmed.contains("::")
            {
                for private_type in [
                    "Operation",
                    "PluginLoadOptions",
                    "OperationMetadata",
                    "FlowService",
                    "SessionService",
                    "EventService",
                    "CapabilityService",
                    "CapabilitySnapshotService",
                    "RuntimeService",
                    "IntentRouter",
                ] {
                    if trimmed.contains(private_type) {
                        violations.push(format!(
                            "{relative}:{}: production interactive source imports private runtime contract `{private_type}` from the coding_session module instead of crate::api: {}",
                            index + 1,
                            trimmed
                        ));
                    }
                }
            }
        }
    }

    // Verify migrated adapters import operation types through crate::api.
    let prompt_task_source =
        fs::read_to_string(scan.crate_root.join("src/interactive/prompt_task.rs"))
            .expect("read prompt_task source");
    let sanitized_prompt_task = sanitize_rust_source(&prompt_task_source);
    assert!(
        sanitized_prompt_task.contains("use crate::api::"),
        "interactive prompt_task must import CodingAgentOperation/CodingAgentOperationOutcome through crate::api per D-16"
    );
    let loop_source = fs::read_to_string(scan.crate_root.join("src/interactive/loop.rs"))
        .expect("read interactive loop source");
    let sanitized_loop = sanitize_rust_source(&loop_source);
    assert!(
        sanitized_loop.contains("use crate::api::"),
        "interactive loop must import public operation projections through crate::api per D-16"
    );

    assert!(
        violations.is_empty(),
        "interactive production source must route operations through CodingAgentSession::run and must not call replaced broad workflow methods, suppress deprecation, or import private runtime contracts:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_adapters_do_not_introduce_switch_active_leaf() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    // CodeGraph discovery found no first-party SwitchActiveLeaf caller. Audit
    // that no production adapter introduces one. The SwitchActiveLeaf operation
    // remains in the public enum for completeness but has no live caller.
    for relative_root in ["src/interactive", "src/protocol", "src/print_mode.rs"] {
        collect_source_violations(
            scan.repo_root(),
            &scan.crate_root.join(relative_root),
            &[],
            &mut violations,
            |line| {
                line.contains(".switch_active_leaf(")
                    || line.contains("CodingAgentOperation::SwitchActiveLeaf")
            },
        );
    }

    assert!(
        violations.is_empty(),
        "production adapters must not introduce a SwitchActiveLeaf caller; CodeGraph found none and the operation has no live first-party caller:\n{}",
        violations.join("\n")
    );
}

#[test]
fn adapters_do_not_access_event_service_directly_for_projection() {
    let scan = SourceScan::new();

    for relative_path in [
        "src/protocol/rpc/commands.rs",
        "src/protocol/rpc/stats.rs",
        "src/protocol/rpc/prompt.rs",
        "src/interactive/loop.rs",
    ] {
        let source = fs::read_to_string(scan.crate_root.join(relative_path))
            .unwrap_or_else(|err| panic!("read {relative_path}: {err}"));
        assert!(
            !source.contains(".event_service."),
            "{relative_path} should project through snapshot/product-event facades instead of accessing EventService directly"
        );
    }
}

#[test]
fn runtime_service_production_paths_require_capability_snapshot() {
    let scan = SourceScan::new();
    let runtime_service_source = fs::read_to_string(
        scan.crate_root
            .join("src/coding_session/runtime_service.rs"),
    )
    .expect("read runtime service source");

    assert_fn_is_test_gated(&runtime_service_source, "fn build_agent_runtime(");
    assert_fn_is_test_gated(
        &runtime_service_source,
        "fn build_agent_runtime_with_plugins(",
    );
    assert_fn_is_test_gated(
        &runtime_service_source,
        "fn build_agent_runtime_with_plugins_and_diagnostics(",
    );
    assert_fn_is_not_test_gated(
        &runtime_service_source,
        "fn build_agent_runtime_with_capabilities(",
    );

    let mut violations = Vec::new();
    collect_source_violations(
        scan.repo_root(),
        &scan.crate_root.join("src/coding_session"),
        &["crates/pi-coding-agent/src/coding_session/runtime_service.rs"],
        &mut violations,
        |line| {
            line.contains(".build_agent_runtime_with_plugins_and_diagnostics(")
                || line.contains(".build_agent_runtime_with_plugins(")
                || line.contains(".build_agent_runtime(")
        },
    );

    assert!(
        violations.is_empty(),
        "production runtime build must route through build_agent_runtime_with_capabilities; permissive compat wrappers must not be called outside runtime_service tests:\n{}",
        violations.join("\n")
    );
}

#[test]
fn plugin_command_paths_use_capability_aware_execution() {
    let scan = SourceScan::new();
    let source = fs::read_to_string(scan.crate_root.join("src/coding_session/mod.rs"))
        .expect("read coding session owner source");

    assert!(
        source.contains("run_command_with_capabilities("),
        "plugin command execution must use run_command_with_capabilities"
    );
    assert!(
        !source.contains(".run_command(\""),
        "plugin command execution must not bypass capability-aware dispatch with bare .run_command( calls"
    );
}

fn assert_fn_is_test_gated(source: &str, signature: &str) {
    let preceding = preceding_non_blank_line(source, signature)
        .unwrap_or_else(|| panic!("signature not found: {signature}"));
    assert!(
        preceding.trim() == "#[cfg(test)]",
        "compat fn `{signature}` must be gated behind #[cfg(test)] so production paths use build_agent_runtime_with_capabilities; preceding line: {preceding:?}"
    );
}

fn assert_fn_is_not_test_gated(source: &str, signature: &str) {
    let preceding = preceding_non_blank_line(source, signature)
        .unwrap_or_else(|| panic!("signature not found: {signature}"));
    assert!(
        preceding.trim() != "#[cfg(test)]",
        "production fn `{signature}` must not be gated behind #[cfg(test)]"
    );
}

fn preceding_non_blank_line<'a>(source: &'a str, signature: &str) -> Option<&'a str> {
    let lines: Vec<&str> = source.lines().collect();
    let idx = lines.iter().position(|line| line.contains(signature))?;
    if idx == 0 {
        return Some("");
    }
    let mut i = idx - 1;
    while i > 0 && lines[i].trim().is_empty() {
        i -= 1;
    }
    Some(lines[i])
}

struct SourceScan {
    crate_root: PathBuf,
    repo_root: PathBuf,
}

impl SourceScan {
    fn new() -> Self {
        let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = crate_root
            .parent()
            .and_then(Path::parent)
            .expect("crate should live under crates/pi-coding-agent")
            .to_path_buf();
        Self {
            crate_root,
            repo_root,
        }
    }

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }
}

fn collect_source_violations(
    repo_root: &Path,
    path: &Path,
    allowed_files: &[&str],
    violations: &mut Vec<String>,
    is_violation: impl Copy + Fn(&str) -> bool,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
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

    let content = fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        if is_violation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

fn add_expectations(
    target: &mut Vec<MethodExpectation>,
    group: &'static str,
    visibility: &'static str,
    test_only: bool,
    names: &[&'static str],
) {
    target.extend(names.iter().map(|name| MethodExpectation {
        name,
        group,
        visibility,
        test_only,
    }));
}

fn format_method_locations(methods: &[&SessionMethod]) -> String {
    methods
        .iter()
        .map(|method| format!("{}:{}-{}", method.file, method.line, method.end_line))
        .collect::<Vec<_>>()
        .join(", ")
}

fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let Ok(metadata) = fs::metadata(root) else {
        return Vec::new();
    };
    if metadata.is_file() {
        return (root.extension().and_then(|extension| extension.to_str()) == Some("rs"))
            .then(|| root.to_path_buf())
            .into_iter()
            .collect();
    }
    let mut files = fs::read_dir(root)
        .expect("read Rust source directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("read Rust source entries")
        .into_iter()
        .flat_map(|entry| rust_files_under(&entry.path()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn relative_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .expect("source should be below repository root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect_coding_agent_session_methods(
    repo_root: &Path,
    path: &Path,
    methods: &mut Vec<SessionMethod>,
) {
    let source = fs::read_to_string(path).expect("read CodingAgentSession source");
    let sanitized = sanitize_rust_source(&source);
    let relative = relative_path(repo_root, path);
    let lines = sanitized.lines().collect::<Vec<_>>();
    let mut in_impl = false;
    let mut depth = 0isize;
    let mut attributes = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !in_impl {
            let impl_suffix = trimmed.strip_prefix("impl CodingAgentSession");
            let starts_impl = impl_suffix.is_some_and(|suffix| {
                let suffix = suffix.trim_start();
                suffix.is_empty() || suffix.starts_with('{')
            });
            let opens_here = impl_suffix.is_some_and(|suffix| suffix.trim_start().starts_with('{'));
            let opens_next = starts_impl
                && !opens_here
                && lines
                    .get(index + 1..)
                    .into_iter()
                    .flatten()
                    .find(|next| !next.trim().is_empty())
                    .is_some_and(|next| next.trim().starts_with('{'));
            if opens_here || opens_next {
                in_impl = true;
                depth = brace_delta(line);
            }
            continue;
        }

        if depth == 1 {
            if trimmed.starts_with("#[") {
                attributes.push(trimmed.to_owned());
            } else if (trimmed.starts_with("pub ") || trimmed.starts_with("pub(crate) "))
                && let Some((visibility, name)) = parse_visible_method_signature(&lines, index)
            {
                let end_index = visible_method_end(&lines, index);
                methods.push(SessionMethod {
                    name,
                    visibility,
                    test_only: attributes
                        .iter()
                        .any(|attribute| attribute == "#[cfg(test)]"),
                    attributes: attributes.clone(),
                    body: lines[index..=end_index].join("\n"),
                    file: relative.clone(),
                    line: index + 1,
                    end_line: end_index + 1,
                });
                attributes.clear();
            } else if !trimmed.is_empty() {
                attributes.clear();
            }
        }
        depth += brace_delta(line);
        if depth == 0 {
            in_impl = false;
            attributes.clear();
        }
    }
}

fn parse_visible_method_signature(lines: &[&str], start: usize) -> Option<(&'static str, String)> {
    let mut signature = String::new();
    for line in lines.iter().skip(start).take(12) {
        if !signature.is_empty() {
            signature.push(' ');
        }
        signature.push_str(line.trim());
        if signature.contains('{') {
            break;
        }
    }
    parse_visible_method(&signature)
}

fn visible_method_end(lines: &[&str], start: usize) -> usize {
    let mut saw_body = false;
    let mut depth = 0isize;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let delta = brace_delta(line);
        if line.contains('{') {
            saw_body = true;
        }
        depth += delta;
        if saw_body && depth == 0 {
            return index;
        }
    }
    panic!(
        "visible method starting at line {} has no complete body",
        start + 1
    );
}

fn unexpected_method_context(method: &SessionMethod) -> String {
    let operation_vocabulary = [
        "CodingAgentOperation",
        "Operation::",
        "PromptTurnOptions",
        "AgentInvocationOptions",
        "AgentTeamOptions",
        "SelfHealingEditRequest",
    ]
    .iter()
    .filter(|token| method.body.contains(*token))
    .copied()
    .collect::<Vec<_>>();
    let forwards_to_run = method.body.contains(".run(") || method.body.contains("Self::run(");
    format!(
        "; targeted context: attributes={:?}, operation_vocabulary={operation_vocabulary:?}, forwards_to_run={forwards_to_run}",
        method.attributes
    )
}

fn alternate_facade_violations(scan: &SourceScan) -> Vec<String> {
    let mut paths = rust_files_under(&scan.crate_root.join("src/coding_session"));
    paths.push(scan.crate_root.join("src/lib.rs"));
    let mut violations = Vec::new();
    for path in paths {
        let relative = relative_path(&scan.repo_root, &path);
        let source = sanitize_rust_source(&fs::read_to_string(&path).expect("read facade source"));
        for (index, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            let public_trait =
                trimmed.starts_with("pub trait ") || trimmed.starts_with("pub(crate) trait ");
            if public_trait
                && (source.contains("CodingAgentSession")
                    || source.contains("CodingAgentOperation"))
                && trimmed.contains("run")
            {
                violations.push(format!(
                    "alternate public trait operation facade at {relative}:{}: {trimmed}",
                    index + 1
                ));
            }
            if trimmed.starts_with("pub use ")
                && trimmed.contains(" as ")
                && [
                    "CodingAgentSession",
                    "CodingAgentOperation",
                    "CodingAgentOperationOutcome",
                    "run",
                ]
                .iter()
                .any(|token| trimmed.contains(token))
            {
                violations.push(format!(
                    "alternate public operation alias at {relative}:{}: {trimmed}",
                    index + 1
                ));
            }
            if let Some(module_name) = public_module_name(trimmed)
                && ["facade", "compat", "workflow"]
                    .iter()
                    .any(|token| module_name.contains(token))
            {
                violations.push(format!(
                    "alternate public operation module `{module_name}` at {relative}:{}",
                    index + 1
                ));
            }
        }
    }
    violations
}

fn public_module_name(line: &str) -> Option<&str> {
    let suffix = line
        .strip_prefix("pub mod ")
        .or_else(|| line.strip_prefix("pub(crate) mod "))?;
    let name = suffix
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .count();
    (name > 0).then_some(&suffix[..name])
}

fn parse_visible_method(line: &str) -> Option<(&'static str, String)> {
    let visibility = if line.starts_with("pub(crate) ") {
        "pub(crate)"
    } else if line.starts_with("pub ") {
        "pub"
    } else {
        return None;
    };
    let fn_index = line.find("fn ")? + 3;
    let name = line[fn_index..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect::<String>();
    (!name.is_empty()).then_some((visibility, name))
}

fn brace_delta(line: &str) -> isize {
    line.chars().fold(0, |depth, character| match character {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

fn assert_direct_cfg_test(source: &str, signature: &str) {
    let lines = source.lines().collect::<Vec<_>>();
    let index = lines
        .iter()
        .position(|line| line.contains(signature))
        .unwrap_or_else(|| panic!("signature not found: {signature}"));
    let mut cursor = index;
    let mut attributes = Vec::new();
    while cursor > 0 {
        cursor -= 1;
        let trimmed = lines[cursor].trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("#[") {
            attributes.push(trimmed);
            continue;
        }
        break;
    }
    assert!(
        attributes.contains(&"#[cfg(test)]"),
        "`{signature}` must be directly gated by #[cfg(test)]; attributes: {attributes:?}"
    );
}

fn line_is_cfg_test_gated(source: &str, line_index: usize) -> bool {
    let lines = source.lines().collect::<Vec<_>>();
    let mut previous = line_index;
    while previous > 0 {
        previous -= 1;
        let trimmed = lines[previous].trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            return true;
        }
        if !trimmed.starts_with("#[") && !trimmed.starts_with("use ") {
            break;
        }
    }

    let mut depth = 0isize;
    let mut test_item_depths = Vec::new();
    let mut pending_test_cfg = false;
    for (index, line) in lines.iter().enumerate().take(line_index + 1) {
        let trimmed = line.trim();
        if trimmed == "#[cfg(test)]" {
            pending_test_cfg = true;
        } else if pending_test_cfg && trimmed.contains('{') {
            test_item_depths.push(depth + 1);
            pending_test_cfg = false;
        } else if pending_test_cfg && trimmed.ends_with(';') {
            if index == line_index {
                return true;
            }
            pending_test_cfg = false;
        }
        depth += brace_delta(line);
        test_item_depths.retain(|item_depth| depth >= *item_depth);
        if index == line_index && (!test_item_depths.is_empty() || pending_test_cfg) {
            return true;
        }
    }
    false
}

fn sanitize_rust_source(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment(usize),
        String,
        Char,
        RawString(usize),
    }

    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            State::Code if byte == b'/' && next == Some(b'/') => {
                output.push_str("  ");
                index += 2;
                state = State::LineComment;
            }
            State::Code if byte == b'/' && next == Some(b'*') => {
                output.push_str("  ");
                index += 2;
                state = State::BlockComment(1);
            }
            State::Code if byte == b'"' => {
                output.push(' ');
                index += 1;
                state = State::String;
            }
            State::Code if byte == b'\'' => {
                output.push(' ');
                index += 1;
                state = State::Char;
            }
            State::Code if byte == b'r' => {
                let mut cursor = index + 1;
                while bytes.get(cursor) == Some(&b'#') {
                    cursor += 1;
                }
                if bytes.get(cursor) == Some(&b'"') {
                    let hashes = cursor - index - 1;
                    output.extend(std::iter::repeat_n(' ', cursor - index + 1));
                    index = cursor + 1;
                    state = State::RawString(hashes);
                } else {
                    output.push(byte as char);
                    index += 1;
                }
            }
            State::Code => {
                output.push(byte as char);
                index += 1;
            }
            State::LineComment => {
                if byte == b'\n' {
                    output.push('\n');
                    state = State::Code;
                } else {
                    output.push(' ');
                }
                index += 1;
            }
            State::BlockComment(depth) if byte == b'/' && next == Some(b'*') => {
                output.push_str("  ");
                index += 2;
                state = State::BlockComment(depth + 1);
            }
            State::BlockComment(depth) if byte == b'*' && next == Some(b'/') => {
                output.push_str("  ");
                index += 2;
                state = if depth == 1 {
                    State::Code
                } else {
                    State::BlockComment(depth - 1)
                };
            }
            State::BlockComment(depth) => {
                output.push(if byte == b'\n' { '\n' } else { ' ' });
                index += 1;
                state = State::BlockComment(depth);
            }
            State::String | State::Char => {
                let quote = matches!(state, State::String)
                    .then_some(b'"')
                    .unwrap_or(b'\'');
                if byte == b'\\' {
                    output.push(' ');
                    if index + 1 < bytes.len() {
                        output.push(if bytes[index + 1] == b'\n' { '\n' } else { ' ' });
                    }
                    index += 2;
                } else {
                    output.push(if byte == b'\n' { '\n' } else { ' ' });
                    index += 1;
                    if byte == quote {
                        state = State::Code;
                    }
                }
            }
            State::RawString(hashes) => {
                if byte == b'"'
                    && bytes.get(index + 1..index + 1 + hashes)
                        == Some(vec![b'#'; hashes].as_slice())
                {
                    output.extend(std::iter::repeat_n(' ', hashes + 1));
                    index += hashes + 1;
                    state = State::Code;
                } else {
                    output.push(if byte == b'\n' { '\n' } else { ' ' });
                    index += 1;
                }
            }
        }
    }
    output
}

fn workspace_path(relative: &str) -> PathBuf {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-coding-agent")
        .to_path_buf();
    repo_root.join(relative)
}

#[test]
fn rpc_running_product_events_do_not_use_unbounded_channels() {
    let prompt_rs = fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/protocol/rpc/prompt.rs",
    ))
    .expect("read rpc prompt source");

    assert!(
        !prompt_rs.contains("mpsc::unbounded_channel"),
        "RPC running product-event forwarding must use bounded queues"
    );
    assert!(
        prompt_rs.contains("RpcProductEventQueue::new()"),
        "RPC prompt forwarding should route through RpcProductEventQueue"
    );
    assert!(
        prompt_rs.contains("RpcQueuedProductEvent::Overflow"),
        "RPC completion drains must handle queued overflow recovery items"
    );
}

#[test]
fn event_receiver_lag_maps_to_snapshot_recovery_error() {
    let event_service_rs = fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/coding_session/event_service.rs",
    ))
    .expect("read event service source");

    assert!(
        event_service_rs.contains("CodingSessionError::EventStreamLag"),
        "broadcast lag must map to event_stream_lag so clients know to request a fresh snapshot"
    );
    assert!(
        !event_service_rs.contains("event receiver lagged by {skipped} events"),
        "lag should not remain a generic resource error"
    );
}

#[test]
fn lifecycle_and_compact_control_authority_remain_narrow_and_private() {
    let scan = SourceScan::new();
    let projection = sanitize_rust_source(
        &fs::read_to_string(
            scan.crate_root
                .join("src/coding_session/public_projection.rs"),
        )
        .expect("read public projection"),
    );
    let shutdown_handle = projection
        .split("pub struct CodingAgentRuntimeShutdownHandle")
        .nth(1)
        .expect("shutdown request handle exists")
        .split("pub enum CodingAgentSubmittedEventDurability")
        .next()
        .expect("durability projection follows shutdown handle");
    assert_eq!(shutdown_handle.matches("pub fn ").count(), 1);
    assert!(shutdown_handle.contains("pub fn request_shutdown(&self)"));
    assert!(shutdown_handle.contains("self.coordinator.request_shutdown();"));
    for forbidden in [
        "pub coordinator",
        "client_id",
        "generation",
        "finish_shutdown",
        "wait_for_active_operation",
        "event_service",
        "emit(",
        "connect",
        "detach",
    ] {
        assert!(
            !shutdown_handle.contains(forbidden),
            "Phase A shutdown handle leaked `{forbidden}` authority"
        );
    }

    let control_path = scan
        .crate_root
        .join("src/coding_session/operation_control.rs");
    let control = fs::read_to_string(&control_path).expect("read operation control source");
    for required in [
        "pub(crate) enum CompactCancellationRejection",
        "NoActiveOperation",
        "ActiveOperationNotCompact",
        "OperationMismatch",
        "pub(crate) struct CompactCancellationHandle",
        "pub(crate) fn cancel(&self, operation_id: &str)",
        "let shared = self.shared.lock()",
        "active.kind != OperationKind::Compact",
        "active.operation_id != operation_id",
        ".cancellation",
        ".cancel();",
    ] {
        assert!(
            control.contains(required),
            "Compact cancellation omitted `{required}`"
        );
    }
    for forbidden in [
        "pub enum CompactCancellationRejection",
        "pub struct CompactCancellationHandle",
        "pub fn cancel(&self",
        "fn cancel(&self, kind:",
        "fn cancel(&self, generation:",
        "fn cancel(&self, operation:",
    ] {
        assert!(
            !control.contains(forbidden),
            "Compact cancellation widened through `{forbidden}`"
        );
    }

    let intent_router = sanitize_rust_source(
        &fs::read_to_string(scan.crate_root.join("src/coding_session/intent_router.rs"))
            .expect("read intent router"),
    );
    assert_eq!(intent_router.matches("enum ControlIntent").count(), 1);
    assert_eq!(intent_router.matches("PromptControl,").count(), 1);
    assert!(!intent_router.contains("CompactControl"));

    let mut escaped = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src")) {
        if path == control_path || path == scan.crate_root.join("src/coding_session/mod.rs") {
            continue;
        }
        let relative = relative_path(&scan.repo_root, &path);
        let production = production_source(&sanitize_rust_source(
            &fs::read_to_string(&path).expect("read production source"),
        ));
        for forbidden in [
            "CompactCancellationHandle",
            "CompactCancellationRejection",
            ".compact_cancellation_handle(",
        ] {
            if production.contains(forbidden) {
                escaped.push(format!("{relative}: leaked `{forbidden}`"));
            }
        }
    }
    assert!(
        escaped.is_empty(),
        "crate-private Compact cancellation authority escaped its owner boundary:\n{}",
        escaped.join("\n")
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdapterOwnership {
    CanonicalOperationCaller,
    StateReplayControlConsumer,
    ApprovedNonRuntimeAdapter,
}

#[derive(Debug, Clone, Copy)]
struct AdapterClassification {
    path: &'static str,
    ownership: AdapterOwnership,
    rationale: &'static str,
}

const ADAPTER_CLASSIFICATIONS: &[AdapterClassification] = &[
    AdapterClassification {
        path: "crates/pi-coding-agent/src/coding_session/client_service.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "runtime-owned client state service, not a product adapter",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/coding_session/mod.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "canonical runtime owner and sole ordinary dispatcher",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/coding_session/public_projection.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "stable state/replay/control contract implementation",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/interactive/app.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "process-facing interactive mode and output owner",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/interactive/event_bridge.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "typed product-event to UI projection",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/interactive/loop.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "interactive connection, replay, and scoped-control owner",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/interactive/prompt_task.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "interactive ordinary-operation task runners",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/lib.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "stable facade plus top-level mode/output router",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/print_mode.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "print-mode Prompt operation adapter",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/protocol/events.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "typed product-event to protocol projection",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/protocol/json_mode.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "JSON-mode Prompt operation and event adapter",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/protocol/rpc/commands.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "short-lived RPC ordinary-operation commands",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/protocol/rpc/events.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "RPC product-event projection wrapper",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/protocol/rpc/prompt.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "select-driven RPC ordinary-operation runners",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/protocol/rpc/state.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "RPC client connection, replay, and output state",
    },
];

const PROHIBITED_SESSION_METHODS: &[&str] = &[
    "invoke_agent",
    "invoke_team",
    "export_current",
    "export_current_html",
    "prompt",
    "compact",
    "self_healing_edit",
    "reload_plugins",
    "run_plugin_command",
    "approve_delegation_confirmation",
    "reject_delegation_confirmation",
    "fork_current_session",
    "summarize_branch",
    "summarize_branch_for_navigation",
];

#[test]
fn adapter_inventory_is_recursive_and_receiver_aware() {
    let scan = SourceScan::new();
    let discovered = discover_adapter_candidates(&scan);
    let classification_violations =
        validate_adapter_classifications(&discovered, ADAPTER_CLASSIFICATIONS);
    assert!(
        classification_violations.is_empty(),
        "adapter discovery/classification ledger drifted:\n{}",
        classification_violations.join("\n")
    );
    assert!(
        discovered.contains("crates/pi-coding-agent/src/interactive/loop.rs"),
        "known interactive adapter is not owned by inventory"
    );
    assert!(
        discovered.contains("crates/pi-coding-agent/src/protocol/rpc/commands.rs"),
        "known RPC adapter is not owned by inventory"
    );

    for classification in ADAPTER_CLASSIFICATIONS {
        let relative = classification.path;
        let path = scan.repo_root.join(&relative);
        let raw = fs::read_to_string(&path).expect("read adapter source");
        let sanitized = sanitize_rust_source(&raw);
        let production = production_source(&sanitized);
        for (line_no, line) in production.lines().enumerate() {
            for method in PROHIBITED_SESSION_METHODS {
                let needle = format!(".{method}(");
                if line.contains(&needle) {
                    panic!(
                        "prohibited workflow call `{method}` in adapter at {relative}:{}: {}",
                        line_no + 1,
                        line.trim()
                    );
                }
            }
        }

        if classification.ownership == AdapterOwnership::CanonicalOperationCaller {
            assert!(
                production_source(&sanitized).contains(".run("),
                "canonical operation caller lacks a production .run( call: {relative}"
            );
        }
    }
}

#[test]
fn runtime_admission_has_no_direct_operation_control_bypass() {
    let scan = SourceScan::new();
    let session_path = scan.crate_root.join("src/coding_session/mod.rs");
    let session_source = fs::read_to_string(&session_path).expect("read coding session source");
    let session_production = production_source(&sanitize_rust_source(&session_source));
    assert_eq!(
        session_production
            .matches("OperationScheduler::admit(")
            .count(),
        3,
        "canonical sync/async dispatchers must all route through typed scheduler admission"
    );
    assert!(
        !session_production.contains("IntentRouter::admit_operation")
            && !session_production.contains("IntentRouter::begin"),
        "legacy router-owned admission entry points must not return to production dispatch"
    );

    let mut violations = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src")) {
        let relative = relative_path(&scan.repo_root, &path);
        let source = fs::read_to_string(&path).expect("read product source");
        let production = production_source(&sanitize_rust_source(&source));
        for (line_no, line) in production.lines().enumerate() {
            let bypass = line.contains("control.begin(")
                || line.contains("operation_control.begin(")
                || line.contains("state.begin(")
                || line.contains(".begin(OperationKind::");
            if !bypass {
                continue;
            }
            let owner = relative.ends_with("src/coding_session/scheduler.rs")
                || relative.ends_with("src/coding_session/operation_control.rs");
            if !owner {
                violations.push(format!("{}:{}: {}", relative, line_no + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "runtime-affecting product code bypasses OperationScheduler admission:\n{}",
        violations.join("\n")
    );
}

#[test]
fn delegated_child_flows_require_scheduler_lineage_admission() {
    let scan = SourceScan::new();
    for relative in [
        "src/coding_session/agent_invocation_flow.rs",
        "src/coding_session/agent_team_flow.rs",
    ] {
        let source = fs::read_to_string(scan.crate_root.join(relative))
            .expect("read delegated child flow source");
        let production = production_source(&sanitize_rust_source(&source));
        assert!(
            production.contains("OperationScheduler::admit_child("),
            "delegated child flow must admit its child capability snapshot through the scheduler: {relative}"
        );
        assert!(
            production.contains("ActorId::ChildOperation("),
            "delegated child flow must construct an explicit parent lineage actor: {relative}"
        );
    }

    let session_source = fs::read_to_string(scan.crate_root.join("src/coding_session/mod.rs"))
        .expect("read coding session source");
    let session_production = production_source(&sanitize_rust_source(&session_source));
    for required in [
        ".invoke_agent_inner(options, snapshot.operation_id.clone())",
        ".invoke_team_inner(options, snapshot.operation_id.clone())",
    ] {
        assert!(
            session_production.contains(required),
            "canonical root dispatch must pass its admitted operation id to child flow: {required}"
        );
    }
}

fn discover_adapter_candidates(scan: &SourceScan) -> HashSet<String> {
    let sources = rust_files_under(&scan.crate_root.join("src"))
        .into_iter()
        .map(|path| {
            let relative = relative_path(&scan.repo_root, &path);
            let source = fs::read_to_string(path).expect("read production source");
            (relative, source)
        })
        .collect::<Vec<_>>();
    let borrowed = sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    discover_adapter_candidates_from_sources(&borrowed)
}

fn discover_adapter_candidates_from_sources(sources: &[(&str, &str)]) -> HashSet<String> {
    sources
        .iter()
        .filter_map(|(path, source)| {
            let sanitized = sanitize_rust_source(source);
            let production = production_source(&sanitized);
            let is_operation_boundary = production.contains("CodingAgentOperation")
                && (production.contains(".run(") || production.contains("session.run("));
            let is_connection_boundary = production.contains("CodingAgentClientConnection")
                || production.contains(".prepare_submission(")
                || production.contains(".reconnect(")
                || production.contains(".acknowledge(");
            let is_event_boundary = (production.contains("ProductEvent")
                || production.contains("CodingAgentProductEvent"))
                && (production.contains("ProtocolEvent")
                    || production.contains("UiEvent")
                    || production.contains("EventAdapter")
                    || production.contains("EventBridge"));
            let is_mode_or_output_boundary = production.contains("CliOutput")
                && (production.contains("run_") && production.contains("mode"));
            (is_operation_boundary
                || is_connection_boundary
                || is_event_boundary
                || is_mode_or_output_boundary)
                .then(|| (*path).to_owned())
        })
        .collect()
}

fn production_source(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut depth = 0isize;
    let mut test_item_depths = Vec::new();
    let mut pending_test_cfg = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed == "#[cfg(test)]" {
            pending_test_cfg = true;
        } else if pending_test_cfg && trimmed.contains('{') {
            test_item_depths.push(depth + 1);
            pending_test_cfg = false;
        } else if pending_test_cfg && trimmed.ends_with(';') {
            pending_test_cfg = false;
        }
        let gated = pending_test_cfg || !test_item_depths.is_empty();
        if !gated {
            output.push_str(line);
        }
        output.push('\n');
        depth += brace_delta(line);
        test_item_depths.retain(|item_depth| depth >= *item_depth);
    }
    output
}

fn validate_adapter_classifications(
    discovered: &HashSet<String>,
    classifications: &[AdapterClassification],
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut classified = HashSet::new();
    for classification in classifications {
        if classification.rationale.trim().is_empty() {
            violations.push(format!(
                "classification has empty rationale: {}",
                classification.path
            ));
        }
        if !classified.insert(classification.path.to_owned()) {
            violations.push(format!(
                "candidate classified more than once: {}",
                classification.path
            ));
        }
    }
    for path in discovered.difference(&classified) {
        violations.push(format!("unclassified adapter candidate: {path}"));
    }
    for path in classified.difference(discovered) {
        violations.push(format!("stale adapter classification: {path}"));
    }
    violations.sort();
    violations
}

#[test]
fn adapter_scanner_fixture_matrix_is_sanitized_and_structural() {
    let fixture = r#"
        // session.prompt("comment")
        let text = ".prompt(";
        let ch = '.';
        #[cfg(test)]
        mod tests { fn hidden(session: &Session) { session.prompt("test"); } }
        session
            .prompt("multiline")
            ;
        (session).prompt("parenthesized");
        other.prompt("legitimate");
    "#;
    let sanitized = sanitize_rust_source(fixture);
    let production = production_source(&sanitized);
    assert_eq!(production.matches(".prompt(").count(), 3);
    assert!(production.contains("session\n            .prompt("));
    assert!(production.contains("(session).prompt("));
    assert!(!production.contains("comment"));
    assert!(production.contains("other.prompt("));
}

#[test]
fn adapter_discovery_fixture_rejects_unclassified_and_stale_ownership() {
    let sources = [
        (
            "src/protocol/new_transport.rs",
            "pub async fn run_new_transport(session: &mut CodingAgentSession) { session.run(CodingAgentOperation::Prompt(todo!())).await; }",
        ),
        (
            "src/protocol/comment_only.rs",
            r#"// CodingAgentSession::run(CodingAgentOperation)
               const TEXT: &str = "CodingAgentClientConnection";"#,
        ),
        (
            "src/helpers/near_miss.rs",
            "fn run_modeled_value() -> usize { 1 }",
        ),
        (
            "src/protocol/test_only.rs",
            "#[cfg(test)] mod tests { fn adapter(session: &mut CodingAgentSession) { session.run(todo!()); } }",
        ),
    ];
    let discovered = discover_adapter_candidates_from_sources(&sources);
    assert_eq!(
        discovered,
        HashSet::from(["src/protocol/new_transport.rs".to_owned()])
    );

    let unclassified = validate_adapter_classifications(&discovered, &[]);
    assert!(
        unclassified
            .iter()
            .any(|violation| violation.contains("unclassified"))
    );

    let stale = validate_adapter_classifications(
        &HashSet::new(),
        &[AdapterClassification {
            path: "src/protocol/removed.rs",
            ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
            rationale: "legacy transport boundary",
        }],
    );
    assert!(stale.iter().any(|violation| violation.contains("stale")));
}

#[test]
fn session_method_inventory_accepts_multiline_impl_and_signature() {
    let fixture = tempfile::tempdir().expect("create session method fixture");
    let source_path = fixture.path().join("session.rs");
    fs::write(
        &source_path,
        r#"
            impl CodingAgentSession
            {
                pub async fn prompt(
                    &mut self,
                    prompt: &str,
                ) -> Result<(), Error> {
                    todo!()
                }
            }
        "#,
    )
    .expect("write session method fixture");

    let mut methods = Vec::new();
    collect_coding_agent_session_methods(fixture.path(), &source_path, &mut methods);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "prompt");
    assert_eq!(methods[0].visibility, "pub");
}
