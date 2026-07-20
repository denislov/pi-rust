use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED_DIRECT_MUTATION_FILES: &[&str] =
    &["crates/pi-coding-agent/tests/api_contract/provider_registry_boundary_guards.rs"];

const ALLOWED_GLOBAL_BUILTIN_REGISTRATION_FILES: &[&str] = &[
    "crates/pi-coding-agent/src/services/runtime.rs",
    "crates/pi-coding-agent/tests/boundaries/product_runtime_boundary_guards.rs",
    "crates/pi-coding-agent/tests/api_contract/provider_registry_boundary_guards.rs",
];

const ALLOWED_GLOBAL_STREAM_MODEL_FILES: &[&str] = &[
    "crates/pi-coding-agent/src/services/runtime.rs",
    "crates/pi-coding-agent/tests/api_contract/provider_registry_boundary_guards.rs",
];
const GLOBAL_PROVIDER_COMPATIBILITY_MARKER: &str = "global provider runtime compatibility example";

#[test]
fn pi_coding_agent_sources_do_not_mutate_the_global_provider_registry() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    for root in scan.roots() {
        collect_direct_registry_mutations(scan.repo_root(), &root, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "pi-coding-agent must use scoped AiClient registration, not global registry mutation:\n{}",
        violations.join("\n")
    );
}

#[test]
fn global_builtin_provider_registration_stays_at_runtime_boundary() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    for root in scan.roots() {
        collect_global_builtin_registration(scan.repo_root(), &root, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "global built-in provider registration must stay behind the runtime compatibility boundary:\n{}",
        violations.join("\n")
    );
}

#[test]
fn global_stream_model_calls_stay_at_runtime_boundary() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    for root in scan.roots() {
        collect_global_stream_model_calls(scan.repo_root(), &root, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "global stream_model calls must stay behind the runtime compatibility boundary:\n{}",
        violations.join("\n")
    );
}

#[test]
fn examples_using_global_provider_runtime_are_explicit_compatibility_examples() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();

    collect_undocumented_global_provider_examples(
        scan.repo_root(),
        &scan.crate_root.join("examples"),
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "examples that use the pi-ai global provider runtime must be explicit compatibility examples:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_and_examples_use_only_categorized_pi_ai_edge_contracts() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();
    let allowed_categories = [
        "auth",
        "client",
        "conversation",
        "model",
        "provider",
        "stream",
        "testing",
    ];

    collect_pi_ai_category_violations(
        &scan.crate_root.join("src"),
        &allowed_categories,
        &mut violations,
    );
    collect_pi_ai_category_violations(
        &scan.crate_root.join("examples"),
        &allowed_categories,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "pi-coding-agent must name only categorized, edge-allowlisted pi-ai contracts:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_and_examples_use_only_categorized_core_edge_contracts() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();
    let allowed_categories = [
        "agent",
        "compaction",
        "execution",
        "flow",
        "resources",
        "testing",
        "tool",
        "transcript",
    ];

    collect_core_category_violations(
        &scan.crate_root.join("src"),
        &allowed_categories,
        &mut violations,
    );
    collect_core_category_violations(
        &scan.crate_root.join("examples"),
        &allowed_categories,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "pi-coding-agent must name only categorized, edge-allowlisted core contracts:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_and_examples_use_only_categorized_tui_edge_contracts() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();
    let allowed_categories = [
        "component",
        "input",
        "render",
        "terminal",
        "testing",
        "theme",
    ];

    collect_tui_category_violations(
        &scan.crate_root.join("src"),
        &allowed_categories,
        &mut violations,
    );
    collect_tui_category_violations(
        &scan.crate_root.join("examples"),
        &allowed_categories,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "pi-coding-agent must name only categorized, edge-allowlisted TUI contracts:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_and_examples_use_only_item_allowlisted_upstream_contracts() {
    let scan = SourceScan::new();
    let mut violations = Vec::new();
    let pi_ai = [
        ("auth", &["ProviderAuthDiagnostic", "env_api_key"][..]),
        ("client", &["AiClient"]),
        (
            "conversation",
            &[
                "AssistantMessage",
                "ContentBlock",
                "Context",
                "Cost",
                "Message",
                "StopReason",
                "Usage",
            ],
        ),
        (
            "model",
            &[
                "Model",
                "ModelCost",
                "ModelInput",
                "all_models",
                "get_providers",
                "lookup_model",
            ],
        ),
        ("provider", &["ApiProvider"]),
        (
            "stream",
            &["AssistantMessageEvent", "EventStream", "StreamOptions"],
        ),
        ("testing", &["FauxProvider", "FauxResponse", "FauxToolCall"]),
    ];
    let core = [
        (
            "agent",
            &[
                "Agent",
                "AgentConfig",
                "AgentEvent",
                "AgentMessage",
                "AgentResources",
                "AgentStream",
                "BeforeToolCallContext",
                "BeforeToolCallResult",
                "CompactionConfig",
                "CompactionSettings",
                "ProviderRequestSnapshot",
                "ProviderStreamer",
                "QueueMode",
                "ThinkingLevel",
            ][..],
        ),
        (
            "compaction",
            &[
                "calculate_context_tokens",
                "estimate_tokens",
                "summarize_with_provider_streamer",
            ],
        ),
        (
            "execution",
            &[
                "ExecOptions",
                "ExecutionEnv",
                "ExecutionOutput",
                "FileError",
                "FileSystem",
                "TruncationLimit",
                "TruncationResult",
                "format_size",
                "truncate_head",
                "truncate_tail",
            ],
        ),
        (
            "resources",
            &[
                "AgentResources",
                "DiagnosticSeverity",
                "PromptTemplate",
                "ResourceDiagnostic",
                "ResourceDiagnosticSeverity",
                "Skill",
                "core_load_skills",
                "core_load_templates",
                "format_skill_invocation",
                "load_prompt_templates",
                "load_skills",
                "parse_command_args",
                "substitute_args",
            ],
        ),
        ("testing", &["InMemoryExecutionEnv"]),
        (
            "tool",
            &[
                "AgentTool",
                "AgentToolOutput",
                "AgentToolResult",
                "ToolExecutionContext",
                "ToolExecutionMode",
                "ToolFn",
                "ToolUpdateCallback",
            ],
        ),
        (
            "transcript",
            &[
                "SessionEntry",
                "SessionHeader",
                "SessionTreeNode",
                "StoredAgentMessage",
                "StoredUsage",
                "StoredUsageCost",
                "create_session_id",
                "create_timestamp",
            ],
        ),
    ];
    let tui = [
        (
            "component",
            &[
                "Component",
                "Editor",
                "Image",
                "Loader",
                "Markdown",
                "OverlayAnchor",
                "OverlayHandle",
                "OverlayMargin",
                "OverlayOptions",
                "SettingItem",
                "SettingsList",
                "SettingsListOptions",
                "SizeValue",
            ][..],
        ),
        (
            "input",
            &[
                "InputEvent",
                "Key",
                "KeyEvent",
                "KeyEventKind",
                "KeyModifiers",
                "KeybindingDefinition",
                "KeybindingsManager",
                "MouseButton",
                "MouseEvent",
                "MouseEventKind",
                "MouseModifiers",
                "StdinBuffer",
                "TUI_KEYBINDINGS",
                "fuzzy_filter_indices",
                "is_key_release",
                "matches_key",
                "parse_key",
            ],
        ),
        (
            "render",
            &[
                "Color",
                "ColorLevel",
                "Constraint",
                "ERROR",
                "FocusRing",
                "Frame",
                "HitMap",
                "HitRegion",
                "Layout",
                "Point",
                "Rect",
                "RenderScheduler",
                "STATUS_IDLE",
                "STATUS_RUNNING",
                "SYSTEM",
                "Style",
                "TOOL_ERROR",
                "TOOL_NAME",
                "Tui",
                "TuiError",
                "USER",
                "color_enabled",
                "paint_with",
                "paint_with_level",
                "truncate_to_width",
                "truncate_to_width_with_ellipsis",
                "visible_width",
                "wrap_text_with_ansi",
            ],
        ),
        (
            "terminal",
            &[
                "ImageProtocol",
                "ProcessTerminal",
                "Terminal",
                "TerminalCapabilities",
                "TerminalMode",
                "TerminalSize",
                "detect_terminal_capabilities_from_env",
            ],
        ),
        ("testing", &["TerminalOp", "VirtualTerminal"]),
        (
            "theme",
            &[
                "MarkdownTheme",
                "ThemePalette",
                "TuiTheme",
                "dark_theme",
                "light_theme",
            ],
        ),
    ];

    for root in [
        scan.crate_root.join("src"),
        scan.crate_root.join("examples"),
    ] {
        collect_api_item_violations(&root, "pi_ai::api::", &pi_ai, &mut violations);
        collect_api_item_violations(&root, "pi_agent_core::api::", &core, &mut violations);
        collect_api_item_violations(&root, "pi_tui::api::", &tui, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "pi-coding-agent named upstream items outside its exact edge allowlists:\n{}",
        violations.join("\n")
    );
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

    fn roots(&self) -> [PathBuf; 2] {
        [self.crate_root.join("src"), self.crate_root.join("tests")]
    }
}

fn collect_pi_ai_category_violations(
    path: &Path,
    allowed_categories: &[&str],
    violations: &mut Vec<String>,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read pi-ai edge directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read pi-ai edge entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_pi_ai_category_violations(&entry.path(), allowed_categories, violations);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let content = fs::read_to_string(path).expect("read pi-ai edge source");
    for (line_index, line) in content.lines().enumerate() {
        let mut remaining = line;
        while let Some(offset) = remaining.find("pi_ai::api::") {
            let suffix = &remaining[offset + "pi_ai::api::".len()..];
            let category: String = suffix
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !allowed_categories.contains(&category.as_str()) {
                violations.push(format!(
                    "{}:{}: category `{}` is not allowed: {}",
                    path.display(),
                    line_index + 1,
                    if category.is_empty() {
                        "<flat-or-grouped>"
                    } else {
                        &category
                    },
                    line.trim()
                ));
            }
            remaining = suffix.get(category.len()..).unwrap_or_default();
        }
    }
}

fn collect_core_category_violations(
    path: &Path,
    allowed_categories: &[&str],
    violations: &mut Vec<String>,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read core edge directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read core edge entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_core_category_violations(&entry.path(), allowed_categories, violations);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let content = fs::read_to_string(path).expect("read core edge source");
    for (line_index, line) in content.lines().enumerate() {
        let mut remaining = line;
        while let Some(offset) = remaining.find("pi_agent_core::api::") {
            let suffix = &remaining[offset + "pi_agent_core::api::".len()..];
            let category: String = suffix
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !allowed_categories.contains(&category.as_str()) {
                violations.push(format!(
                    "{}:{}: category `{}` is not allowed: {}",
                    path.display(),
                    line_index + 1,
                    if category.is_empty() {
                        "<flat-or-grouped>"
                    } else {
                        &category
                    },
                    line.trim()
                ));
            }
            remaining = suffix.get(category.len()..).unwrap_or_default();
        }
    }
}

fn collect_tui_category_violations(
    path: &Path,
    allowed_categories: &[&str],
    violations: &mut Vec<String>,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read TUI edge directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read TUI edge entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_tui_category_violations(&entry.path(), allowed_categories, violations);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let content = fs::read_to_string(path).expect("read TUI edge source");
    for (line_index, line) in content.lines().enumerate() {
        let mut remaining = line;
        while let Some(offset) = remaining.find("pi_tui::") {
            let suffix = &remaining[offset + "pi_tui::".len()..];
            let Some(api_suffix) = suffix.strip_prefix("api::") else {
                violations.push(format!(
                    "{}:{}: root TUI access is forbidden: {}",
                    path.display(),
                    line_index + 1,
                    line.trim()
                ));
                break;
            };
            let category: String = api_suffix
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !allowed_categories.contains(&category.as_str()) {
                violations.push(format!(
                    "{}:{}: category `{}` is not allowed: {}",
                    path.display(),
                    line_index + 1,
                    if category.is_empty() {
                        "<flat-or-grouped>"
                    } else {
                        &category
                    },
                    line.trim()
                ));
            }
            remaining = api_suffix.get(category.len()..).unwrap_or_default();
        }
    }
}

fn collect_api_item_violations(
    path: &Path,
    prefix: &str,
    allowlist: &[(&str, &[&str])],
    violations: &mut Vec<String>,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read upstream item boundary directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read upstream item boundary entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_api_item_violations(&entry.path(), prefix, allowlist, violations);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let content = fs::read_to_string(path).expect("read upstream item boundary source");
    let use_prefix = format!("use {prefix}");
    let mut remaining = content.as_str();
    while let Some(offset) = remaining.find(&use_prefix) {
        let tail = &remaining[offset..];
        let Some(end) = tail.find(';') else {
            violations.push(format!("{}: unterminated `{use_prefix}`", path.display()));
            break;
        };
        let statement = &tail[..end];
        let suffix = &statement[use_prefix.len()..];
        let category_end = suffix.find("::").unwrap_or(suffix.len());
        let category = &suffix[..category_end];
        let imported = suffix.get(category_end + 2..).unwrap_or_default();
        let allowed = allowlist
            .iter()
            .find_map(|(candidate, items)| (*candidate == category).then_some(*items));

        for item in imported
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .filter(|item| !item.is_empty() && !matches!(*item, "as" | "json"))
        {
            if allowed.is_none_or(|items| !items.contains(&item)) {
                violations.push(format!(
                    "{}: `{category}::{item}` is not item-allowlisted",
                    path.display()
                ));
            }
        }
        remaining = &remaining[offset + end + 1..];
    }

    // Also cover fully-qualified expressions that do not have a `use` item.
    let mut remaining = content.as_str();
    while let Some(offset) = remaining.find(prefix) {
        let suffix = &remaining[offset + prefix.len()..];
        let category: String = suffix
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        let after_category = suffix.get(category.len()..).unwrap_or_default();
        if let Some(after_separator) = after_category.strip_prefix("::") {
            let item: String = after_separator
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !item.is_empty() && item != "json" && !after_separator.starts_with('{') {
                let allowed = allowlist
                    .iter()
                    .find_map(|(candidate, items)| (*candidate == category).then_some(*items));
                if allowed.is_none_or(|items| !items.contains(&item.as_str())) {
                    violations.push(format!(
                        "{}: `{category}::{item}` is not item-allowlisted",
                        path.display()
                    ));
                }
            }
        }
        remaining = suffix.get(category.len()..).unwrap_or_default();
    }
}

fn collect_direct_registry_mutations(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    collect_source_violations(
        repo_root,
        path,
        ALLOWED_DIRECT_MUTATION_FILES,
        violations,
        |line| line.contains("registry::register(") || line.contains("registry::unregister("),
    );
}

fn collect_global_builtin_registration(
    repo_root: &Path,
    path: &Path,
    violations: &mut Vec<String>,
) {
    collect_source_violations(
        repo_root,
        path,
        ALLOWED_GLOBAL_BUILTIN_REGISTRATION_FILES,
        violations,
        |line| line.contains("pi_ai::providers::register_builtins()"),
    );
}

fn collect_global_stream_model_calls(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    collect_source_violations(
        repo_root,
        path,
        ALLOWED_GLOBAL_STREAM_MODEL_FILES,
        violations,
        |line| {
            line.contains("pi_ai::stream_model(")
                || line.contains("pi_ai::registry::stream_model(")
                || line.contains("registry::stream_model(")
        },
    );
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
            .expect("read source/test directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read source/test entries");
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

    let content = fs::read_to_string(path).expect("read source/test file");
    for (line_index, line) in content.lines().enumerate() {
        if is_violation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

#[test]
fn product_agent_runtime_build_installs_scoped_ai_client_streamer() {
    let scan = SourceScan::new();
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");

    let start = runtime_service
        .find("fn build_agent_runtime_with_authorization(")
        .expect("runtime build function should exist");
    let end = runtime_service[start..]
        .find("fn apply_tool_policy(")
        .map(|offset| start + offset)
        .expect("runtime build function should be followed by helpers");
    let build_body = &runtime_service[start..end];

    assert!(
        build_body
            .contains("ModelCapability::require(snapshot.model.as_ref(), runtime.profile_id())")
            && build_body
                .contains("scoped_provider_streamer_for_runtime(runtime, model_capability)",),
        "product runtime build should authorize and construct its streamer through the scoped runtime helper"
    );
    assert!(
        build_body.contains("config.provider_streamer = Some(provider_streamer)"),
        "product runtime build should inject the scoped provider streamer"
    );
    assert!(
        !build_body.contains("register_builtin_providers_for_global_runtime();"),
        "product runtime build must not register builtins through the global compatibility helper"
    );
}

#[test]
fn session_admission_installs_the_session_owned_provider_runtime() {
    let scan = SourceScan::new();
    let dispatch = fs::read_to_string(scan.crate_root.join("src/runtime/dispatch.rs"))
        .expect("read operation dispatch source");
    let delegation = fs::read_to_string(
        scan.crate_root
            .join("src/operations/delegation/execution.rs"),
    )
    .expect("read delegation execution source");
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");
    let prompt = fs::read_to_string(scan.crate_root.join("src/operations/prompt/context.rs"))
        .expect("read prompt runtime source");

    assert!(
        dispatch.contains("runtime_host") && dispatch.contains("install_provider_runtime(runtime)"),
        "ordinary session admission must install its provider runtime into operation-local state"
    );
    assert!(
        delegation.contains("runtime_service: &RuntimeService")
            && delegation.contains("runtime_service.install_provider_runtime(runtime)"),
        "delegation approval must install its provider runtime into operation-local state"
    );
    assert!(
        runtime_service.contains("runtime.set_provider_streamer("),
        "RuntimeService must install a streamer backed by its scoped AiClient"
    );
    assert!(
        prompt.contains("provider_streamer: Option<ProviderStreamer>"),
        "operation runtime snapshots must carry the admitted provider streamer"
    );
}

#[test]
fn self_healing_model_repair_uses_scoped_runtime_streaming() {
    let scan = SourceScan::new();
    let self_healing = fs::read_to_string(
        scan.crate_root
            .join("src/operations/self_healing_edit/runner.rs"),
    )
    .expect("read self-healing edit flow source");

    assert!(
        self_healing.contains("stream_model_for_scoped_runtime"),
        "self-healing model repair should use the scoped runtime streaming helper"
    );
    assert!(
        !self_healing.contains("stream_model_for_global_runtime"),
        "self-healing model repair must not call the global streaming compatibility helper"
    );
}

#[test]
fn scoped_runtime_streaming_helper_uses_ai_client_without_global_stream_model() {
    let scan = SourceScan::new();
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");

    let start = runtime_service
        .find("fn stream_model_for_scoped_runtime(")
        .expect("scoped runtime streaming helper should exist");
    let end = runtime_service[start..]
        .find("impl RuntimeService")
        .map(|offset| start + offset)
        .expect("scoped runtime helper should be before RuntimeService impl");
    let helper_body = &runtime_service[start..end];

    assert!(
        helper_body.contains("AiClient::new()"),
        "scoped runtime helper should create a scoped AiClient"
    );
    assert!(
        helper_body.contains("ai_client.register_builtins()"),
        "scoped runtime helper should install builtins into the scoped AiClient"
    );
    assert!(
        !helper_body.contains("pi_ai::stream_model("),
        "scoped runtime helper must not stream through the global pi-ai compatibility helper"
    );
}

#[test]
fn summary_product_flows_use_scoped_runtime_streamer() {
    let scan = SourceScan::new();
    let manual_compaction =
        fs::read_to_string(scan.crate_root.join("src/operations/compaction/runner.rs"))
            .expect("read manual compaction flow source");
    let branch_summary = fs::read_to_string(
        scan.crate_root
            .join("src/operations/branch_summary/runner.rs"),
    )
    .expect("read branch summary flow source");

    assert!(
        manual_compaction.contains("summarize_with_provider_streamer"),
        "manual compaction should use the provider-streamer-aware summarizer"
    );
    assert!(
        manual_compaction.contains("ModelCapability::require(")
            && manual_compaction
                .contains("self.options.runtime(),\n                model_capability,"),
        "manual compaction should stream through the capability-scoped runtime provider streamer"
    );
    assert!(
        branch_summary.contains("summarize_with_provider_streamer"),
        "branch summary should use the provider-streamer-aware summarizer"
    );
    assert!(
        branch_summary.contains("ModelCapability::require(")
            && branch_summary.contains("runtime,\n                    model_capability,"),
        "branch summary should stream through the capability-scoped runtime provider streamer"
    );
}

#[test]
fn runtime_service_exposes_reusable_scoped_provider_streamer() {
    let scan = SourceScan::new();
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");

    assert!(
        runtime_service.contains("fn scoped_provider_streamer_for_runtime("),
        "runtime service should expose one scoped provider streamer helper for product model-call paths"
    );
    assert!(
        runtime_service.contains("ProviderStreamer"),
        "scoped provider streamer helper should return the core ProviderStreamer boundary type"
    );
    assert!(
        runtime_service.contains("AiClient::new()"),
        "scoped provider streamer helper should create a scoped pi_ai::api::client::AiClient"
    );
    assert!(
        runtime_service.contains("ai_client.register_builtins()"),
        "scoped provider streamer helper should install builtins into the scoped AiClient"
    );
}

#[test]
fn runtime_service_no_longer_exposes_global_stream_model_helper() {
    let scan = SourceScan::new();
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");

    assert!(
        !runtime_service.contains("fn stream_model_for_global_runtime("),
        "runtime service should not expose a product global streaming helper"
    );
    assert!(
        !runtime_service.contains("pi_ai::stream_model("),
        "runtime service should not call the pi-ai global streaming compatibility helper"
    );
}

#[test]
fn runtime_global_builtin_registration_boundary_is_retired() {
    let scan = SourceScan::new();
    let runtime_service = fs::read_to_string(scan.crate_root.join("src/services/runtime.rs"))
        .expect("read runtime service source");

    assert!(
        !runtime_service.contains("fn register_builtin_providers_for_global_runtime()"),
        "pi-coding-agent should not retain a global built-in provider registration compatibility helper"
    );
    assert!(
        !runtime_service.contains("pi_ai::providers::register_builtins()"),
        "pi-coding-agent product runtime should use scoped AiClient::register_builtins() instead of the global helper"
    );
}

fn collect_undocumented_global_provider_examples(
    repo_root: &Path,
    path: &Path,
    violations: &mut Vec<String>,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read examples directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read examples entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_undocumented_global_provider_examples(repo_root, &entry.path(), violations);
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
    let content = fs::read_to_string(path).expect("read example file");
    if uses_global_provider_runtime(&content)
        && (!content.contains(GLOBAL_PROVIDER_COMPATIBILITY_MARKER)
            || !content.contains("#[allow(deprecated)]"))
    {
        violations.push(format!(
            "{relative}: add `{GLOBAL_PROVIDER_COMPATIBILITY_MARKER}` docs and #[allow(deprecated)]"
        ));
    }
}

fn uses_global_provider_runtime(content: &str) -> bool {
    content.lines().any(|line| {
        line.contains("registry::register(")
            || line.contains("registry::unregister(")
            || line.contains("registry::stream_model(")
            || line.contains("pi_ai::registry::register(")
            || line.contains("pi_ai::registry::unregister(")
            || line.contains("pi_ai::registry::stream_model(")
            || line.contains("pi_ai::stream_model(")
    })
}
