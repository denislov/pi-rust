use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED_GLOBAL_STREAM_MODEL_FILES: &[&str] = &["crates/pi-agent-core/src/ai_runtime.rs"];
const GLOBAL_PROVIDER_COMPATIBILITY_MARKER: &str = "global provider runtime compatibility example";

#[test]
fn global_stream_model_calls_stay_behind_ai_runtime_boundary() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-agent-core");
    let src_root = crate_root.join("src");
    let mut violations = Vec::new();

    collect_global_stream_model_calls(repo_root, &src_root, &mut violations);

    assert!(
        violations.is_empty(),
        "global stream_model calls must stay behind pi-agent-core ai_runtime boundary:\n{}",
        violations.join("\n")
    );
}

#[test]
fn examples_using_global_provider_runtime_are_explicit_compatibility_examples() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-agent-core");
    let examples_root = crate_root.join("examples");
    let mut violations = Vec::new();

    collect_undocumented_global_provider_examples(repo_root, &examples_root, &mut violations);

    assert!(
        violations.is_empty(),
        "examples that use the pi-ai global provider runtime must be explicit compatibility examples:\n{}",
        violations.join("\n")
    );
}

fn collect_global_stream_model_calls(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
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
            collect_global_stream_model_calls(repo_root, &entry.path(), violations);
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
    if ALLOWED_GLOBAL_STREAM_MODEL_FILES.contains(&relative.as_str()) {
        return;
    }

    let content = fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        if line.contains("pi_ai::stream_model(") || line.contains("pi_ai::registry::stream_model(")
        {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
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
