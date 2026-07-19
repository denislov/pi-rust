//! Product runtime ownership and bypass prevention boundaries.

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
fn legacy_extension_and_generic_flow_implementations_cannot_return() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("coding-agent crate should be in the workspace");
    let manifest =
        fs::read_to_string(crate_root.join("Cargo.toml")).expect("read pi-coding-agent manifest");
    assert!(
        !manifest.contains("mlua"),
        "Lua runtime dependency must stay deleted"
    );

    let forbidden_extension_symbols = [
        "PluginRegistry",
        "PluginSource",
        "ToolProvider",
        "CommandProvider",
        "HookProvider",
        "UiProvider",
        "KeybindProvider",
        "LuaToolProvider",
        "LuaCommandProvider",
    ];
    let forbidden_flow_symbols = [
        "pi_agent_core::api::flow",
        "crate::flow",
        "FlowNode",
        "FlowOutcome",
        "FlowRunOptions",
        "FlowService",
        "AgentTurnFlow",
    ];

    let mut violations = Vec::new();
    for root in [
        crate_root.join("src"),
        repo_root.join("crates/pi-agent-core/src"),
    ] {
        for path in rust_files_under(&root) {
            let source = fs::read_to_string(&path).expect("read production Rust source");
            for forbidden in forbidden_extension_symbols
                .iter()
                .chain(forbidden_flow_symbols.iter())
            {
                if source.contains(forbidden) {
                    violations.push(format!(
                        "{} contains {forbidden}",
                        relative_path(repo_root, &path)
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "legacy extension/Flow code returned:\n{}",
        violations.join("\n")
    );

    for removed in [
        "crates/pi-agent-core/src/flow",
        "crates/pi-coding-agent/src/services/flow.rs",
    ] {
        assert!(
            !repo_root.join(removed).exists(),
            "removed path returned: {removed}"
        );
    }
    for operation in [
        "agent_invocation",
        "branch_summary",
        "compaction",
        "export",
        "plugin_load",
        "prompt",
        "self_healing_edit",
        "team_invocation",
    ] {
        assert!(
            !crate_root
                .join(format!("src/operations/{operation}/flow.rs"))
                .exists(),
            "typed operation must not regain a Flow wrapper: {operation}"
        );
    }
}

#[test]
fn extension_host_handles_are_lease_only_and_service_free() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/extensions/host.rs"))
            .expect("read extension Host API handles");

    assert!(source.contains("OperationCapabilityLease"));
    for family in [
        "WorkspaceHostHandle",
        "ModelHostHandle",
        "ProcessHostHandle",
        "UiHostHandle",
    ] {
        assert!(
            source.contains(family),
            "missing typed Host API family {family}"
        );
    }
    for forbidden in [
        "SessionService",
        "RuntimeService",
        "PluginService",
        "EventService",
        "AiClient",
        "Repository",
        "OperationControl",
        "WorkflowService",
        "Sender<",
        "Receiver<",
        "Arc<dyn",
    ] {
        assert!(
            !source.contains(forbidden),
            "extension Host API handles must not expose raw authority: {forbidden}"
        );
    }
}

#[test]
fn extension_handler_targets_are_data_only_and_cannot_decode_core_authority() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let target_source = fs::read_to_string(crate_root.join("src/contributions/mod.rs"))
        .expect("read contribution target source");
    let manifest_source = fs::read_to_string(crate_root.join("src/extensions/manifest.rs"))
        .expect("read extension manifest source");
    let wit = fs::read_to_string(crate_root.join("../../contracts/extensions/0.1.0/extension.wit"))
        .expect("read extension WIT");

    assert!(target_source.contains("Core(CoreHandlerRef)"));
    assert!(target_source.contains("Extension(ExtensionHandlerRef)"));
    assert!(!target_source.contains("Deserialize"));
    for forbidden in [
        "Arc<dyn",
        "Box<dyn",
        "SessionService",
        "PluginService",
        "Repository",
        "AiClient",
        "Sender<",
        "Receiver<",
    ] {
        assert!(
            !target_source.contains(forbidden),
            "handler target leaked executable authority: {forbidden}"
        );
    }
    for extension_controlled in [&manifest_source, &wit] {
        for forbidden in [
            "CoreHandlerRef",
            "core_handler",
            "core-handler",
            "coreHandler",
        ] {
            assert!(
                !extension_controlled.contains(forbidden),
                "extension contract can address core handlers: {forbidden}"
            );
        }
    }
}

#[test]
fn session_store_failure_controls_remain_test_only() {
    let scan = SourceScan::new();
    let store_path = scan.crate_root.join("src/session/repository.rs");
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
        "AppendOutbox",
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

    let test_support_source =
        fs::read_to_string(scan.crate_root.join("src/runtime/facade/test_support.rs"))
            .expect("read coding session test-support source");
    let session_sanitized = sanitize_rust_source(&test_support_source);
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
    for path in rust_files_under(&scan.crate_root.join("src/runtime")) {
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
    for path in rust_files_under(&scan.crate_root.join("src/runtime")) {
        collect_coding_agent_session_methods(&scan.repo_root, &path, &mut methods);
    }

    let mut expected = Vec::new();
    add_expectations(
        &mut expected,
        "canonical dispatcher",
        "pub",
        false,
        &["run", "submit"],
    );
    add_expectations(
        &mut expected,
        "trusted-host extension lifecycle",
        "pub",
        false,
        &[
            "create_extension_staging_directory",
            "install_extension_staged",
            "activate_extensions",
        ],
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
        "connect_client",
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
            "capability_control",
            "shutdown",
            "snapshot",
            "connect",
            "capabilities",
            "view",
            "recovery_pending",
            "resolve_recovery",
            "retry_recovery",
            "agent_profiles",
            "team_profiles",
            "profile_diagnostics",
            "pending_delegation_confirmations",
            "pending_tool_authorizations",
            "decide_tool_authorization",
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
            "ui_snapshot",
            "product_events_after",
            "prompt_control_handle",
            "tool_authorization_control",
            "plugin_commands",
            "plugin_ui_actions",
            "plugin_ui_dialogs",
            "plugin_keybindings",
            "install_submission_lease",
            "resolve_recovery_with_authority",
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
            "emit_diagnostic_for_tests",
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
        if relative == "crates/pi-coding-agent/tests/events_snapshot/event_boundary_guards.rs" {
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
            == "crates/pi-coding-agent/tests/events_snapshot/event_boundary_guards.rs"
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

    for relative_root in [
        "src/adapters/interactive",
        "src/protocol",
        "src/adapters/print.rs",
    ] {
        collect_source_violations(
            scan.repo_root(),
            &scan.crate_root.join(relative_root),
            &[],
            &mut violations,
            |line| {
                line.contains("Agent::new(")
                    || line.contains("Agent::with_messages(")
                    || line.contains("use pi_agent_core::api::agent::Agent;")
                    || line.contains("pi_agent_core::api::agent::{Agent,")
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
fn production_json_and_print_use_canonical_headless_boundary() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    // JSON/print adapter files must submit Prompt operations through
    // CodingAgentSession::run instead of deprecated broad workflow methods, and
    // must not suppress deprecation warnings in production source. Test-only
    // allowances inside #[cfg(test)] modules are preserved.
    let adapter_files = ["src/adapters/json/mod.rs", "src/adapters/print.rs"];
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
        assert!(
            sanitized.contains("open_headless_prompt_session"),
            "{relative_path} must delegate session preparation to the app/session owner"
        );
        for forbidden in [
            "CodingAgentSession::create(",
            "CodingAgentSession::open(",
            "CodingAgentSession::open_or_create(",
            "CodingAgentSession::list(",
            "CodingAgentSession::fork_session(",
            "CodingAgentSession::non_persistent(",
            "resolve_session_dir(",
        ] {
            assert!(
                !sanitized.contains(forbidden),
                "{relative_path} must not own headless session preparation: {forbidden}"
            );
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

    for path in rust_files_under(&scan.crate_root.join("src/adapters/rpc")) {
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
            for forbidden in [
                "CodingAgentSession::create(",
                "CodingAgentSession::open(",
                "CodingAgentSession::open_or_create(",
                "CodingAgentSession::list(",
                "CodingAgentSession::fork_session(",
                "CodingAgentSession::non_persistent(",
                "resolve_session_dir(",
            ] {
                if trimmed.contains(forbidden) {
                    violations.push(format!(
                        "{relative}:{}: production RPC source owns session preparation instead of delegating to app/session: {}",
                        index + 1,
                        trimmed
                    ));
                }
            }
        }
    }

    let prompt_source = fs::read_to_string(scan.crate_root.join("src/adapters/rpc/prompt.rs"))
        .expect("read RPC prompt source");
    let commands_source = fs::read_to_string(scan.crate_root.join("src/adapters/rpc/commands.rs"))
        .expect("read RPC commands source");
    assert!(prompt_source.contains("open_new_runtime_session"));
    assert!(prompt_source.contains("runtime_session_root"));
    assert!(commands_source.contains("open_new_runtime_session"));
    let state_source = fs::read_to_string(scan.crate_root.join("src/adapters/rpc/state.rs"))
        .expect("read RPC state source");
    assert!(state_source.contains("resolve_runtime_defaults"));
    for forbidden in [
        "config::load_config(",
        "select_model(",
        "config::auth::resolve_api_key(",
    ] {
        assert!(
            !state_source.contains(forbidden),
            "RPC state must consume app-owned runtime defaults: {forbidden}"
        );
    }
    let stats_source = fs::read_to_string(scan.crate_root.join("src/adapters/rpc/stats.rs"))
        .expect("read RPC stats source");
    assert!(stats_source.contains("CodingAgentCapabilities::for_session_write_operation"));
    assert!(!stats_source.contains("crate::runtime::control"));
    assert!(!stats_source.contains("PluginCapabilities"));

    assert!(
        violations.is_empty(),
        "RPC production source must route operations through CodingAgentSession::run/submit and must not call replaced broad workflow methods or suppress deprecation:\n{}",
        violations.join("\n")
    );

    let prompt = fs::read_to_string(scan.crate_root.join("src/adapters/rpc/prompt.rs"))
        .expect("read RPC prompt owner");
    let prompt = sanitize_rust_source(&prompt);
    assert!(prompt.contains("session.submit(CodingAgentOperation::InvokeAgent"));
    assert!(prompt.contains("session.submit(CodingAgentOperation::InvokeTeam"));
    assert!(!prompt.contains("session.run(CodingAgentOperation::InvokeAgent"));
    assert!(!prompt.contains("session.run(CodingAgentOperation::InvokeTeam"));

    let rpc_commands = fs::read_to_string(scan.crate_root.join("src/adapters/rpc/commands.rs"))
        .expect("read RPC command owner");
    let rpc_commands = sanitize_rust_source(&rpc_commands);
    assert!(rpc_commands.contains("session.submit(CodingAgentOperation::PluginCommand"));
    assert!(!rpc_commands.contains("session.run(CodingAgentOperation::PluginCommand"));

    let interactive_loop =
        fs::read_to_string(scan.crate_root.join("src/adapters/interactive/loop.rs"))
            .expect("read interactive operation owner");
    let interactive_loop = sanitize_rust_source(&interactive_loop);
    assert!(interactive_loop.contains(".submit(CodingAgentOperation::PluginCommand"));

    let interactive_task = fs::read_to_string(
        scan.crate_root
            .join("src/adapters/interactive/prompt_task.rs"),
    )
    .expect("read interactive task owner");
    let interactive_task = sanitize_rust_source(&interactive_task);
    assert!(!interactive_task.contains(".run(CodingAgentOperation::PluginCommand"));
}

#[test]
fn production_rpc_projects_the_public_client_connection_without_authority_mirrors() {
    let scan = SourceScan::new();
    let state_path = scan.crate_root.join("src/adapters/rpc/state.rs");
    let prompt_path = scan.crate_root.join("src/adapters/rpc/prompt.rs");
    let state = fs::read_to_string(&state_path).expect("read RPC state");
    let prompt = fs::read_to_string(&prompt_path).expect("read RPC prompt");
    let state_production = state.split("#[cfg(").next().unwrap();
    let prompt_production = prompt.split("#[cfg(").next().unwrap();

    assert!(state_production.contains("client_connection"));
    assert!(state_production.contains("CodingAgentClientConnection"));
    assert!(prompt_production.contains("connection.reconnect_from_cursor("));
    assert!(prompt_production.contains("connection.acknowledge("));
    assert!(prompt_production.contains("connection.prepare_submission("));
    assert!(prompt_production.contains(".run(CodingAgentOperation::ApproveDelegation"));
    assert!(prompt_production.contains(".run(operation)"));

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

    for path in rust_files_under(&scan.crate_root.join("src/adapters/interactive")) {
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
                    "OperationDescriptor",
                    "OperationExecution",
                    "WorkflowService",
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
    let prompt_task_source = fs::read_to_string(
        scan.crate_root
            .join("src/adapters/interactive/prompt_task.rs"),
    )
    .expect("read prompt_task source");
    let sanitized_prompt_task = sanitize_rust_source(&prompt_task_source);
    assert!(
        sanitized_prompt_task.contains("use crate::api::"),
        "interactive prompt_task must import CodingAgentOperation/CodingAgentOperationOutcome through crate::api per D-16"
    );
    assert!(sanitized_prompt_task.contains("open_interactive_session"));
    for forbidden in [
        "CodingAgentSession::create(",
        "CodingAgentSession::open(",
        "CodingAgentSession::open_or_create(",
        "CodingAgentSession::list(",
        "CodingAgentSession::fork_session(",
        "CodingAgentSession::non_persistent(",
        "resolve_session_dir(",
    ] {
        assert!(
            !sanitized_prompt_task.contains(forbidden),
            "interactive prompt_task must delegate session preparation to app/session: {forbidden}"
        );
    }
    let session_actions_source = fs::read_to_string(
        scan.crate_root
            .join("src/adapters/interactive/session_actions.rs"),
    )
    .expect("read interactive session actions source");
    let session_actions_production = session_actions_source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    for required in [
        "hydrate_interactive_session_target",
        "list_interactive_session_hydrations",
        "clone_interactive_session",
        "interactive_session_tree",
        "export_interactive_session_html",
    ] {
        assert!(session_actions_production.contains(required));
    }
    for forbidden in [
        "CodingAgentSession::hydrate(",
        "CodingAgentSession::list(",
        "CodingAgentSession::clone_session(",
        "CodingAgentSession::tree_view(",
        "CodingAgentSession::export_session_html(",
    ] {
        assert!(
            !session_actions_production.contains(forbidden),
            "interactive session_actions must project app-owned session commands: {forbidden}"
        );
    }
    let interactive_app_source =
        fs::read_to_string(scan.crate_root.join("src/adapters/interactive/app.rs"))
            .expect("read interactive app source");
    assert!(interactive_app_source.contains("resolve_cli_context_from_options"));
    assert!(interactive_app_source.contains("resolve_profile_registry"));
    assert!(interactive_app_source.contains("configured_model_choices"));
    assert!(interactive_app_source.contains("rotation_model_choices"));
    for forbidden in [
        "config::resolve_paths(",
        "ProfileRegistry::load(",
        "discover_context_files(",
        "config::auth::resolve_api_key(",
        "parse_model_rotation(",
        "pi_ai::api::model::all_models(",
    ] {
        assert!(
            !interactive_app_source.contains(forbidden),
            "interactive app must consume app-owned config/resource resolution: {forbidden}"
        );
    }
    let interactive_loop_source =
        fs::read_to_string(scan.crate_root.join("src/adapters/interactive/loop.rs"))
            .expect("read interactive loop source");
    assert!(interactive_loop_source.contains("resolve_provider_api_key"));
    assert!(!interactive_loop_source.contains("config::auth::resolve_api_key("));
    assert!(interactive_loop_source.contains("persist_global_settings"));
    assert!(!interactive_loop_source.contains("merge_and_save_settings("));
    let interactive_commands_source =
        fs::read_to_string(scan.crate_root.join("src/adapters/interactive/commands.rs"))
            .expect("read interactive commands source");
    assert!(interactive_commands_source.contains("save_provider_api_key"));
    assert!(interactive_commands_source.contains("remove_provider_auth"));
    assert!(!interactive_commands_source.contains("config::resolve_paths("));
    assert!(!interactive_commands_source.contains(".auth.save("));
    let interactive_root_source =
        fs::read_to_string(scan.crate_root.join("src/adapters/interactive/root.rs"))
            .expect("read interactive root source");
    assert!(interactive_root_source.contains("profile_registry_for_cwd"));
    assert!(!interactive_root_source.contains("config::resolve_paths("));
    assert!(!interactive_root_source.contains("ProfileRegistry::load("));
    let loop_source = fs::read_to_string(scan.crate_root.join("src/adapters/interactive/loop.rs"))
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
    for relative_root in [
        "src/adapters/interactive",
        "src/protocol",
        "src/adapters/print.rs",
    ] {
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
        "src/adapters/rpc/commands.rs",
        "src/adapters/rpc/stats.rs",
        "src/adapters/rpc/prompt.rs",
        "src/adapters/interactive/loop.rs",
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
    let runtime_service_source =
        fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
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
        &scan.crate_root.join("src"),
        &["crates/pi-coding-agent/src/services/runtime.rs"],
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
fn builtin_filesystem_and_shell_tools_are_bound_from_frozen_handles() {
    let scan = SourceScan::new();
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");
    let tools = fs::read_to_string(scan.crate_root.join("src/tools/mod.rs"))
        .expect("read built-in tool registry source");

    assert!(
        runtime_service.contains("bind_builtin_tool_to_capabilities("),
        "RuntimeService must bind reserved built-in tools from the admitted capability snapshot"
    );
    assert!(runtime_service.contains("snapshot.filesystem.as_ref()"));
    assert!(runtime_service.contains("snapshot.shell.as_ref()"));

    for (name, constructor) in [
        ("read", "filesystem::read::read_tool"),
        ("write", "filesystem::write::write_tool"),
        ("edit", "filesystem::edit::edit_tool"),
        ("grep", "filesystem::grep::grep_tool"),
        ("find", "filesystem::find::find_tool"),
        ("ls", "filesystem::ls::ls_tool"),
        ("bash", "shell::bash_tool"),
    ] {
        assert!(
            tools.contains(&format!("\"{name}\" =>")),
            "reserved built-in tool `{name}` must have an explicit frozen-handle binding"
        );
        assert!(
            tools.contains(constructor),
            "reserved built-in tool `{name}` must be reconstructed by `{constructor}`"
        );
    }
}

#[test]
fn model_provider_paths_require_the_frozen_model_handle() {
    let scan = SourceScan::new();
    let owners = [
        "src/services/runtime.rs",
        "src/operations/compaction/runner.rs",
        "src/operations/branch_summary/runner.rs",
        "src/operations/self_healing_edit/mod.rs",
    ];

    for relative in owners {
        let source = fs::read_to_string(scan.crate_root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        assert!(
            source.contains("ModelCapability::require("),
            "{relative} must authorize model/provider access with the frozen model handle"
        );
    }

    let runtime = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");
    assert!(runtime.contains("model_capability: &ModelCapability"));
    assert!(!runtime.contains("scoped_provider_streamer_for_runtime(runtime);"));
}

#[test]
fn product_events_use_operation_bound_capability_generation() {
    let scan = SourceScan::new();
    let intent = fs::read_to_string(scan.crate_root.join("src/runtime/intent.rs"))
        .expect("read operation permit source");
    let control = fs::read_to_string(scan.crate_root.join("src/runtime/control.rs"))
        .expect("read operation control source");
    let event = fs::read_to_string(scan.crate_root.join("src/services/event.rs"))
        .expect("read event service source");
    let recovery = fs::read_to_string(scan.crate_root.join("src/session/service.rs"))
        .expect("read startup recovery source");
    let publish_start = event
        .find("fn publish(")
        .expect("find canonical ProductEvent publish function");
    let publish_end = event[publish_start..]
        .find("#[cfg(test)]")
        .map(|offset| publish_start + offset)
        .expect("find end of ProductEvent publish function");
    let publish = &event[publish_start..publish_end];

    assert_eq!(
        intent
            .matches("bind_capability_generation(execution.capability_generation)")
            .count(),
        2,
        "root and child permits must bind their permit-owned execution generation"
    );
    assert!(control.contains("register_operation_event_context("));
    assert!(control.contains("clear_operation_event_context_if("));
    assert!(publish.contains("operation_event_contexts"));
    assert!(!control.contains("register_operation_capability_generation("));
    assert!(!publish.contains("operation_capability_generations"));
    assert!(
        !publish.contains("state.capability_generation"),
        "ProductEvent generation must not use the coordinator's current generation"
    );
    assert!(recovery.contains("runtime_generation.capability_generation"));
    assert!(event.contains("struct ProductEventEmissionContext"));
    assert!(event.contains("publish_recovery_event("));
    assert!(event.contains("capability_generation: capability_generation"));
    assert!(!event.contains("emit_with_capability_generation("));
}

#[test]
fn plugin_command_paths_use_capability_aware_execution() {
    let scan = SourceScan::new();
    let source = fs::read_to_string(scan.crate_root.join("src/runtime/execution.rs"))
        .expect("read runtime-owned operation execution source");

    assert!(
        source.contains("run_command_with_capabilities("),
        "plugin command execution must use run_command_with_capabilities"
    );
    assert!(
        !source.contains(".run_command(\""),
        "plugin command execution must not bypass capability-aware dispatch with bare .run_command( calls"
    );
}

#[test]
fn agent_tools_receive_the_admitted_operation_scope() {
    let scan = SourceScan::new();
    let runtime = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");
    let shell = fs::read_to_string(scan.crate_root.join("src/tools/shell.rs"))
        .expect("read shell tool source");

    assert!(runtime.contains("config.tool_execution_scope = Some(snapshot.operation_id.clone())"));
    assert!(shell.contains("context.cancel_token().clone()"));
    assert!(shell.contains("cancel_token.cancelled()"));
}

#[test]
fn capability_revocation_is_generation_scoped_and_closes_stale_admission() {
    let scan = SourceScan::new();
    let scheduler = fs::read_to_string(scan.crate_root.join("src/runtime/scheduler.rs"))
        .expect("read scheduler source");
    let control = fs::read_to_string(scan.crate_root.join("src/runtime/control.rs"))
        .expect("read operation control source");
    let projection = fs::read_to_string(scan.crate_root.join("src/runtime/client/projection.rs"))
        .expect("read capability control source");
    let prompt = fs::read_to_string(scan.crate_root.join("src/operations/prompt/mod.rs"))
        .expect("read prompt operation source");

    assert!(scheduler.contains("begin_root_with_capability_generation("));
    assert!(scheduler.contains("begin_child_with_capability_generation("));
    assert!(
        control.contains("generation < self.snapshot_coordinator.current_capability_generation()")
    );
    assert!(control.contains("cancel_capability_generations_before("));
    assert!(projection.contains("pub fn revoke_older_operations("));
    assert!(projection.contains("RequestCancelOlderOperations"));
    assert!(projection.contains("cancellation_requested_operation_ids"));
    assert!(prompt.contains("context.set_operation_cancellation(cancellation)"));
}

#[test]
fn session_mutating_operation_owners_require_frozen_write_capability() {
    let scan = SourceScan::new();
    let owners = [
        ("src/operations/prompt/mod.rs", 1usize),
        ("src/operations/compaction/mod.rs", 1),
        ("src/operations/branch_summary/mod.rs", 1),
        ("src/operations/self_healing_edit/mod.rs", 1),
        ("src/operations/plugin_load/mod.rs", 1),
        ("src/operations/delegation/execution.rs", 1),
        ("src/runtime/dispatch.rs", 5),
    ];

    for (relative, expected) in owners {
        let source = fs::read_to_string(scan.crate_root.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        assert_eq!(
            source.matches("SessionWriteCapability::require(").count(),
            expected,
            "{relative} must guard each session-mutating operation entry with the frozen write capability"
        );
    }
}

#[test]
fn prompt_hooks_execute_only_through_the_frozen_plugin_capability() {
    let scan = SourceScan::new();
    let source = fs::read_to_string(scan.crate_root.join("src/operations/prompt/context.rs"))
        .expect("read prompt operation context");

    assert!(source.contains("run_prompt_hook_with_capabilities("));
    assert!(!source.contains("self.plugin_service.run_prompt_hook("));
    assert!(source.contains("capability_snapshot"));
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
    if root.file_name().and_then(|name| name.to_str()) == Some("internal_tests") {
        return Vec::new();
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
    let mut paths = rust_files_under(&scan.crate_root.join("src/runtime"));
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
                if relative.ends_with("src/runtime/mod.rs") && module_name == "facade" {
                    continue;
                }
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
        "crates/pi-coding-agent/src/adapters/rpc/prompt.rs",
    ))
    .expect("read rpc prompt source");
    let state_rs = fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/adapters/rpc/state.rs",
    ))
    .expect("read rpc state source");

    assert!(
        !prompt_rs.contains("UnboundedSender<ProductEvent")
            && !state_rs.contains("UnboundedSender<ProductEvent")
            && !state_rs.contains("unbounded_channel::<ProductEvent"),
        "RPC ProductEvent forwarding must not use unbounded channels"
    );
    assert!(
        state_rs.contains("RpcProductEventQueue::new()")
            && state_rs.contains("session_events: Option<RpcProductEventReceiver>"),
        "RPC session event pump should route through one bounded RpcProductEventQueue"
    );
    assert!(
        state_rs
            .contains("background_completion_tx: mpsc::UnboundedSender<RpcBackgroundCompletion>")
    );
    assert!(
        prompt_rs.contains("RpcQueuedProductEvent::Overflow"),
        "RPC completion drains must handle queued overflow recovery items"
    );
}

#[test]
fn event_receiver_lag_maps_to_snapshot_recovery_error() {
    let event_service_rs = fs::read_to_string(workspace_path(
        "crates/pi-coding-agent/src/services/event.rs",
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
fn lifecycle_and_operation_control_authority_remain_narrow_and_identity_scoped() {
    let scan = SourceScan::new();
    let projection = sanitize_rust_source(
        &fs::read_to_string(scan.crate_root.join("src/runtime/client/projection.rs"))
            .expect("read public projection"),
    );
    let shutdown_handle = projection
        .split("pub struct CodingAgentRuntimeShutdownHandle")
        .nth(1)
        .expect("shutdown request handle exists")
        .split("pub struct CodingAgentCapabilityControl")
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

    let capability_control = projection
        .split("pub struct CodingAgentCapabilityControl")
        .nth(1)
        .expect("capability revocation control exists")
        .split("pub enum CodingAgentSubmittedEventDurability")
        .next()
        .expect("durability projection follows capability control");
    assert_eq!(capability_control.matches("pub fn ").count(), 1);
    assert!(capability_control.contains("pub fn revoke_older_operations(&self)"));
    assert!(capability_control.contains("RequestCancelOlderOperations"));
    for forbidden in [
        "pub coordinator",
        "pub operation_control",
        "pub event_service",
    ] {
        assert!(
            !capability_control.contains(forbidden),
            "capability control leaked `{forbidden}` authority"
        );
    }

    let operation_control = projection
        .split("pub struct CodingAgentOperationControl")
        .nth(1)
        .expect("operation control exists")
        .split("pub struct CodingAgentPromptControl")
        .next()
        .expect("prompt control follows operation control");
    assert_eq!(operation_control.matches("pub fn ").count(), 1);
    assert!(operation_control.contains("pub fn abort("));
    for forbidden in [
        "pub coordinator",
        "pub fn steer(",
        "pub fn follow_up(",
        "CancellationToken",
    ] {
        assert!(
            !operation_control.contains(forbidden),
            "operation control leaked `{forbidden}` authority"
        );
    }

    let control_path = scan.crate_root.join("src/runtime/control.rs");
    let control = fs::read_to_string(&control_path).expect("read operation control source");
    assert!(control.contains("struct ActiveOperationIdentity"));
    assert!(control.contains("operation_id: String"));
    assert!(control.contains("active.operation_id == self.operation_id"));
    assert!(!control.contains("CompactCancellationHandle"));
    assert!(!control.contains("CompactCancellationRejection"));

    let snapshot = fs::read_to_string(scan.crate_root.join("src/runtime/snapshot.rs"))
        .expect("read snapshot coordinator");
    for required in [
        "struct OperationCancellationBinding",
        "owner: ClientHandle",
        "operation_cancellations: Mutex<HashMap<String, OperationCancellationBinding>>",
        "cancellation: OperationCancellationHandle",
        "cancellation_bindings.get(operation_id)",
        "active.owner.id != handle.id",
        "active.cancellation.request()",
        "clear_operation_cancellation_if",
    ] {
        assert!(
            snapshot.contains(required),
            "operation cancellation omitted `{required}`"
        );
    }
    assert!(
        !snapshot.contains("cancellation: CancellationToken"),
        "snapshot coordinator must route through owner-side cancellation authority"
    );

    let intent_router = sanitize_rust_source(
        &fs::read_to_string(scan.crate_root.join("src/runtime/intent.rs"))
            .expect("read intent router"),
    );
    assert_eq!(intent_router.matches("enum ControlIntent").count(), 1);
    assert_eq!(intent_router.matches("PromptControl,").count(), 1);
    assert!(!intent_router.contains("CompactControl"));

    assert!(!snapshot.contains("pub fn bind_operation_cancellation"));
    assert!(!snapshot.contains("pub fn clear_operation_cancellation_if"));
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
        path: "crates/pi-coding-agent/src/runtime/facade.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "canonical runtime owner and sole ordinary dispatcher",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/runtime/facade/connection.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "session-owned connection, snapshot, replay, and lifecycle facade",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/runtime/client/projection.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "stable state/replay/control contract implementation",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/runtime/execution.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "runtime-owned operation task and scoped control-owner binding",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/interactive/app.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "process-facing interactive mode and output owner",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/interactive/event_bridge.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "typed product-event to UI projection",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/interactive/loop.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "interactive connection, replay, and scoped-control owner",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/interactive/prompt_task.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "interactive ordinary-operation task runners",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/lib.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "stable categorized facade only",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/app/cli/mod.rs",
        ownership: AdapterOwnership::ApprovedNonRuntimeAdapter,
        rationale: "top-level CLI mode selection and output routing",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/print.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "print-mode Prompt operation adapter",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/events.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "typed product-event to protocol projection",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/json/mod.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "JSON-mode Prompt operation and event adapter",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/rpc/commands.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "short-lived RPC ordinary-operation commands",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/rpc/events.rs",
        ownership: AdapterOwnership::StateReplayControlConsumer,
        rationale: "RPC product-event projection wrapper",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/rpc/prompt.rs",
        ownership: AdapterOwnership::CanonicalOperationCaller,
        rationale: "select-driven RPC ordinary-operation runners",
    },
    AdapterClassification {
        path: "crates/pi-coding-agent/src/adapters/rpc/state.rs",
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
        discovered.contains("crates/pi-coding-agent/src/adapters/interactive/loop.rs"),
        "known interactive adapter is not owned by inventory"
    );
    assert!(
        discovered.contains("crates/pi-coding-agent/src/adapters/rpc/commands.rs"),
        "known RPC adapter is not owned by inventory"
    );

    for classification in ADAPTER_CLASSIFICATIONS {
        let relative = classification.path;
        let path = scan.repo_root.join(relative);
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
    let session_path = scan.crate_root.join("src/runtime/facade.rs");
    let session_source = fs::read_to_string(&session_path).expect("read coding session source");
    let dispatch_path = scan.crate_root.join("src/runtime/dispatch.rs");
    let dispatch_source =
        fs::read_to_string(&dispatch_path).expect("read operation dispatch source");
    let session_production = production_source(&sanitize_rust_source(&session_source));
    let dispatch_production = production_source(&sanitize_rust_source(&dispatch_source));
    let scheduler_admission_count = session_production
        .matches("OperationScheduler::admit(")
        .count()
        + dispatch_production
            .matches("OperationScheduler::admit(")
            .count();
    assert_eq!(
        scheduler_admission_count, 3,
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
            let owner = relative.ends_with("src/runtime/scheduler.rs")
                || relative.ends_with("src/runtime/control.rs");
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
fn production_runtime_has_no_permanently_disabled_fallbacks() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    for path in rust_files_under(&scan.crate_root.join("src")) {
        let relative = relative_path(&scan.repo_root, &path);
        let source = fs::read_to_string(&path).expect("read product source");
        let production = production_source(&sanitize_rust_source(&source));
        for (line_no, line) in production.lines().enumerate() {
            if line.contains("cfg(any())") || line.contains("cfg(all(any()") {
                violations.push(format!("{}:{}: {}", relative, line_no + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production runtime contains permanently disabled fallback paths:\n{}",
        violations.join("\n")
    );
}

#[test]
fn durable_operation_paths_consume_admitted_identity_without_regeneration() {
    let scan = SourceScan::new();
    let transaction = fs::read_to_string(scan.crate_root.join("src/session/transaction.rs"))
        .expect("read transaction source");
    assert!(
        transaction.contains("begin_admitted_with_runtime_generation"),
        "production transaction construction must expose the admitted-identity entry point"
    );
    assert!(
        transaction.matches("next_root_operation_id()").count() == 1,
        "only the test-only transaction compatibility constructor may mint an identity"
    );
    assert_direct_cfg_test(&transaction, "pub(crate) fn begin_with_runtime_generation(");

    for relative in [
        "src/operations/agent_invocation/runner.rs",
        "src/operations/team_invocation/runner.rs",
    ] {
        let source = fs::read_to_string(scan.crate_root.join(relative))
            .expect("read invocation flow source");
        let production = source;
        assert!(
            !production.contains("with_scheduler_parent_operation_id"),
            "invocation contexts must receive their execution identity at construction: {relative}"
        );
        assert!(
            !production.contains("next_root_operation_id()")
                && !production.contains("next_child_operation_id()"),
            "invocation flows must request child identities from the scheduler: {relative}"
        );
        assert!(
            production.contains("OperationScheduler::allocate_child_operation_id()"),
            "invocation flows must allocate child identities through the scheduler: {relative}"
        );
    }

    let scheduler = fs::read_to_string(scan.crate_root.join("src/runtime/scheduler.rs"))
        .expect("read scheduler source");
    assert!(
        scheduler.contains("pub(crate) fn allocate_child_operation_id()"),
        "the scheduler must own child operation identity allocation"
    );

    let delegation = fs::read_to_string(
        scan.crate_root
            .join("src/operations/delegation/execution.rs"),
    )
    .expect("read delegation execution source");
    assert!(
        delegation.contains(
            "let approval_operation_id = parent_capability_snapshot.operation_id.clone();"
        ),
        "delegation approval persistence must reuse the admitted approval identity"
    );
    assert!(
        !delegation.contains("next_root_operation_id()")
            && !delegation.contains("next_child_operation_id()"),
        "delegation execution must not mint an unadmitted approval identity"
    );

    let dispatch = fs::read_to_string(scan.crate_root.join("src/runtime/dispatch.rs"))
        .expect("read operation dispatch source");
    let execution = fs::read_to_string(scan.crate_root.join("src/runtime/execution.rs"))
        .expect("read runtime-owned execution source");
    assert_eq!(
        dispatch
            .matches("commit_execution(operation_permit.execution())")
            .count()
            + execution
                .matches("commit_execution(operation_permit.execution())")
                .count(),
        4,
        "every dispatcher must hand the admitted execution to submission finalization"
    );
    let submission = fs::read_to_string(scan.crate_root.join("src/runtime/submission.rs"))
        .expect("read submission finalization source");
    assert!(
        submission.contains("execution: Option<OperationExecution>"),
        "submission finalization must retain the admitted execution"
    );
    assert!(
        submission.contains("decision: &FinalizationDecision"),
        "submission finalization must consume the supervisor's immutable decision"
    );
    assert!(
        !submission.contains("pub(super) operation_id: Option<String>"),
        "submission finalization must not reconstruct identity from a detached string"
    );
    assert_eq!(
        dispatch.matches(".freeze(&execution, &result)").count()
            + execution.matches(".freeze(&execution, &result)").count(),
        4,
        "every dispatcher must freeze finalization through OperationSupervisor"
    );
    assert!(
        !submission.contains("fn submitted_terminal_status("),
        "submission projection must not retain a second terminal classifier"
    );

    let mut allocator_violations = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src")) {
        let relative = relative_path(&scan.repo_root, &path);
        let source = fs::read_to_string(&path).expect("read product source");
        for (line_index, line) in source.lines().enumerate() {
            if line.contains(".next_root_operation_id()")
                && !relative.ends_with("/src/runtime/admission.rs")
                && !relative.ends_with("/src/session/transaction.rs")
                && !relative.ends_with("/src/session/id.rs")
            {
                allocator_violations.push(format!(
                    "{relative}:{}: {}",
                    line_index + 1,
                    line.trim()
                ));
            }
            if line.contains(".next_child_operation_id()")
                && !relative.ends_with("/src/runtime/scheduler.rs")
                && !relative.ends_with("/src/session/id.rs")
            {
                allocator_violations.push(format!(
                    "{relative}:{}: {}",
                    line_index + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        allocator_violations.is_empty(),
        "operation identity allocator ownership was bypassed:\n{}",
        allocator_violations.join("\n")
    );
}

#[test]
fn runtime_host_owner_graph_and_first_writer_command_are_explicit() {
    let scan = SourceScan::new();
    let facade = fs::read_to_string(scan.crate_root.join("src/runtime/facade.rs"))
        .expect("read runtime facade source");
    assert!(
        facade.contains(
            "pub struct CodingAgentSession {\n    pub(super) runtime_host: RuntimeHost,\n}"
        ),
        "CodingAgentSession must remain a facade over one RuntimeHost composition root"
    );
    for legacy_field in [
        "pub(super) persistence: SessionPersistence",
        "pub(super) operation_control: OperationControl",
        "pub(super) event_service: EventService",
        "pub(super) snapshot_coordinator: Arc<SnapshotCoordinator>",
        "pub(super) client_service: ClientService",
    ] {
        assert!(
            !facade.contains(legacy_field),
            "facade must not retain owner authority: {legacy_field}"
        );
    }

    let owners = fs::read_to_string(scan.crate_root.join("src/runtime/owners.rs"))
        .expect("read runtime owners source");
    for owner in [
        "struct RuntimeHost",
        "struct OperationSupervisor",
        "struct EventHub",
        "struct ClientProjectionCoordinator",
    ] {
        assert!(owners.contains(owner), "missing runtime owner: {owner}");
    }
    assert!(
        owners.contains("finalizer: OperationFinalizer"),
        "OperationSupervisor must own terminal decision freezing"
    );
    let session_coordinator =
        fs::read_to_string(scan.crate_root.join("src/runtime/session_coordinator.rs"))
            .expect("read session coordinator source");
    for contract in [
        "struct SessionCoordinator",
        "struct SessionWriterCommand",
        "enum SessionMutation",
        "enum SessionWriterReply",
        "fn execute_writer_command(",
        "operation_id: String",
        "capability_generation: CapabilityGeneration",
    ] {
        assert!(
            session_coordinator.contains(contract),
            "missing writer protocol contract: {contract}"
        );
    }

    let dispatch = fs::read_to_string(scan.crate_root.join("src/runtime/dispatch.rs"))
        .expect("read operation dispatch source");
    assert!(
        dispatch.contains(".execute_writer_command("),
        "session mutation dispatch must enter the SessionCoordinator writer protocol"
    );
    assert!(
        !dispatch.contains("set_default_agent_profile_id("),
        "default-profile mutation must not bypass the writer command protocol"
    );
    for bypass in [
        "confirmation::reject_pending(",
        "session_navigation::fork(",
        "session_navigation::switch_active_leaf(",
        "session_navigation::set_tree_label(",
    ] {
        assert!(
            !dispatch.contains(bypass),
            "session mutation must not bypass the writer command protocol: {bypass}"
        );
    }
    let delegation_execution = fs::read_to_string(
        scan.crate_root
            .join("src/operations/delegation/execution.rs"),
    )
    .expect("read delegation execution source");
    assert!(
        delegation_execution.contains("session_coordinator: &mut SessionCoordinator"),
        "delegation approval must receive the narrow session owner"
    );
    assert!(
        !delegation_execution.contains("persistence: &mut SessionPersistence")
            && !delegation_execution
                .contains("pending_confirmations: &mut PendingDelegationConfirmationQueue"),
        "delegation approval must not split persistence and pending-queue authority"
    );

    let mut workflow_host_leaks = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src/operations")) {
        let source = fs::read_to_string(&path).expect("read workflow source");
        if source.contains("RuntimeHost") {
            workflow_host_leaks.push(relative_path(&scan.repo_root, &path));
        }
    }
    assert!(
        workflow_host_leaks.is_empty(),
        "RuntimeHost must not become a workflow service locator:\n{}",
        workflow_host_leaks.join("\n")
    );
}

#[test]
fn turn_transaction_stages_through_typed_writer_commands_without_repository_handles() {
    let scan = SourceScan::new();
    let transaction = fs::read_to_string(scan.crate_root.join("src/session/transaction.rs"))
        .expect("read transaction source");
    let struct_start = transaction
        .find("pub(crate) struct TurnTransaction")
        .expect("TurnTransaction declaration");
    let impl_start = transaction[struct_start..]
        .find("impl<G, C> TurnTransaction")
        .map(|offset| struct_start + offset)
        .expect("TurnTransaction implementation");
    let fields = &transaction[struct_start..impl_start];
    assert!(fields.contains("writer: SessionTransactionWriter"));
    assert!(fields.contains("session_id: String"));
    assert!(
        !fields.contains("store: SessionLogStore") && !fields.contains("handle: SessionHandle"),
        "workflow transaction must not retain raw repository authority"
    );
    for command in [
        "SessionTransactionWriterCommand::InitializeSession",
        "SessionTransactionWriterCommand::Checkpoint",
        "SessionTransactionWriterCommand::Finalize",
        "SessionTransactionWriterCommand::CommitSessionMutation",
    ] {
        assert!(
            transaction.contains(command),
            "missing typed transaction writer command: {command}"
        );
    }
    for transport in [
        "SESSION_WRITER_REGISTRY",
        "Weak<SessionTransactionWriterInner>",
        "writer_registry_key",
        "owners: AtomicUsize",
        "SessionWriterOwnerLease",
        "release_owner",
        "manifest_snapshot",
        "snapshot: Arc<Mutex<SessionManifest>>",
        "const SESSION_TRANSACTION_WRITER_CAPACITY: usize",
        "sync_channel::<SessionTransactionWriterEnvelope>",
        ".try_send(envelope)",
        "session transaction writer queue is full",
        "impl Drop for SessionTransactionWriterInner",
        "worker.join()",
        "let mut handle = handle",
        "execute_writer_command(&store, &mut handle, envelope.command)",
        "refresh_writer_handle(store, handle)",
        "outbox_records: Vec<DurableOutboxRecordCandidate>",
        "append_events_and_outbox(handle, &events, &outbox_records)",
    ] {
        assert!(
            transaction.contains(transport),
            "missing bounded transaction writer transport contract: {transport}"
        );
    }
    let repository = fs::read_to_string(scan.crate_root.join("src/session/repository.rs"))
        .expect("read session repository source");
    for durable_cursor_contract in [
        ".commit(committed_through_session_sequence)",
        "outbox commit requires at least one sequenced session event",
        "references an event outside its commit batch",
    ] {
        assert!(
            repository.contains(durable_cursor_contract),
            "repository must own durable outbox cursor assignment: {durable_cursor_contract}"
        );
    }
    let service = fs::read_to_string(scan.crate_root.join("src/session/service.rs"))
        .expect("read session service source");
    assert!(
        service.contains("transaction_writer: SessionTransactionWriter"),
        "one SessionService owner must share one transaction writer transport"
    );
    assert!(
        service.contains("SessionTransactionWriter::new(store.clone(), handle.clone())"),
        "SessionService construction must acquire the canonical per-session writer"
    );
    let event_writer_start = service
        .find("pub(crate) struct SessionEventWriter")
        .expect("SessionEventWriter declaration");
    let event_writer_impl = service[event_writer_start..]
        .find("impl SessionEventWriter")
        .map(|offset| event_writer_start + offset)
        .expect("SessionEventWriter implementation");
    let event_writer_fields = &service[event_writer_start..event_writer_impl];
    assert!(event_writer_fields.contains("writer: SessionTransactionWriter"));
    assert!(event_writer_fields.contains("session_id: String"));
    assert!(event_writer_fields.contains("committed_session_sequence: Arc<AtomicU64>"));
    assert!(
        !event_writer_fields.contains("store: SessionLogStore")
            && !event_writer_fields.contains("handle: SessionHandle"),
        "authorization event writer must not retain raw repository authority"
    );
    assert!(
        service.contains("self.writer.append_checkpoint_events_with_receipt(events)")
            && service
                .contains("observe_commit_receipt(&self.committed_session_sequence, receipt)"),
        "authorization durable facts must use the shared bounded writer and retain its commit cursor"
    );
    let production_service = production_source(&sanitize_rust_source(&service));
    assert!(
        !production_service.contains(".store.append_events")
            && !production_service.contains(".store.update_manifest"),
        "SessionService must route live, bootstrap, and copy-target durable mutations through the writer"
    );
    let connection = fs::read_to_string(scan.crate_root.join("src/runtime/facade/connection.rs"))
        .expect("read runtime facade connection source");
    assert!(
        connection.contains("session_service.committed_session_sequence()")
            && !connection.contains("session_service.replay()"),
        "snapshot projection must consume the writer-derived commit cursor without replaying"
    );
    assert!(
        !production_service.contains("self.handle =")
            && !production_service.contains("target.handle ="),
        "repository handles remain read authority; mutable owner state must stay in the writer"
    );
    let repository_path = scan.crate_root.join("src/session/repository.rs");
    let transaction_path = scan.crate_root.join("src/session/transaction.rs");
    let mut durable_write_bypasses = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src")) {
        if path == repository_path || path == transaction_path {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read product source");
        let production = production_source(&sanitize_rust_source(&source));
        if production.contains(".append_events(") || production.contains(".update_manifest(") {
            durable_write_bypasses.push(relative_path(&scan.repo_root, &path));
        }
    }
    assert!(
        durable_write_bypasses.is_empty(),
        "production durable session writes must enter the writer owner; bypasses:\n{}",
        durable_write_bypasses.join("\n")
    );
    for mutation in [
        "set_tree_label",
        "set_default_agent_profile_id",
        "switch_active_leaf",
        "apply_startup_recovery",
        "append_durable_session_event",
    ] {
        assert!(
            service.contains(mutation),
            "missing expected migrated session mutation: {mutation}"
        );
    }
    let connection = fs::read_to_string(scan.crate_root.join("src/runtime/facade/connection.rs"))
        .expect("read runtime shutdown source");
    let drain = connection
        .find(".wait_for_active_operation_to_drain()")
        .expect("runtime shutdown drains operations");
    let close = connection
        .find(".session_coordinator.shutdown_writer()?")
        .expect("runtime shutdown closes session writer");
    let publish = connection
        .find(".emit_runtime_shutdown()")
        .expect("runtime shutdown publishes after writer close");
    assert!(
        drain < close && close < publish,
        "shutdown must drain operations, close/join the writer, then publish shutdown"
    );

    let mut workflow_repository_leaks = Vec::new();
    for path in rust_files_under(&scan.crate_root.join("src/operations")) {
        let source = fs::read_to_string(&path).expect("read workflow source");
        let production = production_source(&sanitize_rust_source(&source));
        if production.contains("SessionLogStore") || production.contains("SessionHandle") {
            workflow_repository_leaks.push(relative_path(&scan.repo_root, &path));
        }
    }
    assert!(
        workflow_repository_leaks.is_empty(),
        "workflow sources must not acquire raw session repository handles:\n{}",
        workflow_repository_leaks.join("\n")
    );
}

#[test]
fn delegated_child_flows_require_scheduler_lineage_admission() {
    let scan = SourceScan::new();
    for relative in [
        "src/operations/agent_invocation/mod.rs",
        "src/operations/team_invocation/mod.rs",
    ] {
        let source = fs::read_to_string(scan.crate_root.join(relative))
            .expect("read delegated operation wrapper source");
        assert!(source.contains("parent_capability_snapshot: OperationCapabilitySnapshot"));
        assert!(source.contains(".with_parent_capability_snapshot(parent_capability_snapshot)"));
    }

    for relative in [
        "src/operations/agent_invocation/runner.rs",
        "src/operations/team_invocation/runner.rs",
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
        assert!(
            production.contains("&self.operation_control"),
            "child admission must use the session runtime OperationControl owner: {relative}"
        );
        assert!(
            production.contains("delegation::execution::execute_agent(")
                && production.contains("delegation::execution::execute_team("),
            "nested delegation wrappers must use the shared admitted execution owner: {relative}"
        );
    }

    let scheduler = fs::read_to_string(scan.crate_root.join("src/runtime/scheduler.rs"))
        .expect("read scheduler source");
    let scheduler = production_source(&sanitize_rust_source(&scheduler));
    assert!(scheduler.contains(".begin_child_with_capability_generation("));
    assert!(scheduler.contains("OperationPermit::child("));

    let control = fs::read_to_string(scan.crate_root.join("src/runtime/control.rs"))
        .expect("read operation control source");
    for contract in [
        "children: Vec<ActiveChildOperation>",
        "owner_released: bool",
        "cancel_descendants",
        "remove_released_children_without_descendants",
        "remove_released_roots_without_descendants",
    ] {
        assert!(
            control.contains(contract),
            "operation control must own child lifetime contract `{contract}`"
        );
    }

    let delegation = fs::read_to_string(
        scan.crate_root
            .join("src/operations/delegation/execution.rs"),
    )
    .expect("read delegation execution source");
    let delegation = production_source(&sanitize_rust_source(&delegation));
    assert_eq!(
        delegation
            .matches("OperationScheduler::admit_child(")
            .count(),
        2
    );

    let dispatch_source = fs::read_to_string(scan.crate_root.join("src/runtime/dispatch.rs"))
        .expect("read operation dispatch source");
    let dispatch_production = production_source(&sanitize_rust_source(&dispatch_source));
    for entrypoint in [
        "crate::operations::agent_invocation::run(",
        "crate::operations::team_invocation::run(",
    ] {
        let call = dispatch_production.find(entrypoint).unwrap_or_else(|| {
            panic!("missing canonical child operation entrypoint: {entrypoint}")
        });
        let call_region = &dispatch_production[call..dispatch_production.len().min(call + 512)];
        assert!(
            call_region.contains("snapshot.operation_id.clone()"),
            "canonical root dispatch must pass its admitted operation id to child flow: {entrypoint}"
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

#[test]
fn production_code_does_not_import_testing_facades() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    for path in rust_files_under(&scan.crate_root.join("src")) {
        let relative = relative_path(&scan.repo_root, &path);
        if relative.contains("/src/internal_tests/") {
            continue;
        }
        let raw = fs::read_to_string(&path).expect("read product source");
        let production = production_source(&sanitize_rust_source(&raw));
        for (line_index, line) in production.lines().enumerate() {
            if line.contains("::api::testing") {
                violations.push(format!("{relative}:{}: {}", line_index + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "test-support facades must not enter product production code:\n{}",
        violations.join("\n")
    );
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
