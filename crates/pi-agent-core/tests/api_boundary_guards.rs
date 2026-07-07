use std::fs;
use std::path::PathBuf;

#[test]
fn root_public_modules_are_marked_migration_private() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source =
        fs::read_to_string(crate_root.join("src/lib.rs")).expect("read pi-agent-core lib.rs");
    let mut violations = Vec::new();
    let mut previous_non_empty = "";

    for (line_index, line) in lib_source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(module) = trimmed.strip_prefix("pub mod ").and_then(|module| {
            module
                .trim_end_matches(';')
                .trim_end_matches('{')
                .split_whitespace()
                .next()
        }) && module != "api"
            && previous_non_empty != "#[doc(hidden)]"
        {
            violations.push(format!(
                "src/lib.rs:{}: root module `{module}` should be marked #[doc(hidden)] migration-private",
                line_index + 1
            ));
        }

        if !trimmed.is_empty() {
            previous_non_empty = trimmed;
        }
    }

    assert!(
        violations.is_empty(),
        "pi_agent_core::api is the stable facade; root public modules must be explicitly migration-private:\n{}",
        violations.join("\n")
    );
}
