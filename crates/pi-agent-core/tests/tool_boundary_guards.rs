use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn agent_core_tool_runtime_has_no_coding_agent_product_imports() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-agent-core");
    let mut violations = Vec::new();

    collect_product_imports(repo_root, &crate_root.join("src"), &mut violations);

    assert!(
        violations.is_empty(),
        "pi-agent-core tool/runtime source must not import pi-coding-agent product ownership:\n{}",
        violations.join("\n")
    );
}

fn collect_product_imports(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
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
            collect_product_imports(repo_root, &entry.path(), violations);
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
    let content = fs::read_to_string(path).expect("read source file");
    for (line_index, line) in content.lines().enumerate() {
        if line.contains("pi_coding_agent")
            || line.contains("CodingAgentSession")
            || line.contains("CodingAgentEvent")
        {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}
