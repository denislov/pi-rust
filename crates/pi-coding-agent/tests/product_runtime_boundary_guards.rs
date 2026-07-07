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
