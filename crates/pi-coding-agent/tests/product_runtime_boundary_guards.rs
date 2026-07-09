use std::fs;
use std::path::{Path, PathBuf};

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
                    || line.contains("use pi_agent_core::Agent;")
                    || line.contains("use pi_agent_core::{Agent,")
                    || line.contains("use pi_agent_core::{ Agent,")
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
