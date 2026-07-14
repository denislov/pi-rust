use std::fs;
use std::path::Path;

#[test]
fn production_and_examples_do_not_use_global_provider_runtime() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    collect_global_provider_calls(&crate_root.join("src"), &mut violations);
    collect_global_provider_calls(&crate_root.join("examples"), &mut violations);

    assert!(
        violations.is_empty(),
        "pi-agent-core must receive a scoped ProviderStreamer instead of using global pi-ai runtime:\n{}",
        violations.join("\n")
    );
}

#[test]
fn missing_provider_streamer_fails_explicitly() {
    let source = include_str!("../src/ai_runtime.rs");
    assert!(source.contains("provider streamer is required"));
    assert!(source.contains("missing_provider_streamer(model)"));
    assert!(!source.contains("pi_ai::stream_model("));
    assert!(!source.contains("pi_ai::registry::"));
}

fn collect_global_provider_calls(path: &Path, violations: &mut Vec<String>) {
    let metadata = fs::metadata(path).expect("provider boundary path should exist");
    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read provider boundary directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read provider boundary entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_global_provider_calls(&entry.path(), violations);
        }
        return;
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let content = fs::read_to_string(path).expect("read provider boundary source");
    for (index, line) in content.lines().enumerate() {
        if line.contains("pi_ai::stream_model(")
            || line.contains("pi_ai::registry::stream_model(")
            || line.contains("pi_ai::registry::register(")
            || line.contains("pi_ai::registry::unregister(")
        {
            violations.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
        }
    }
}
