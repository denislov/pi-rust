use std::fs;
use std::path::PathBuf;

#[test]
fn root_public_modules_are_marked_migration_private() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let lines = lib_source.lines().collect::<Vec<_>>();
    let mut violations = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub mod ") {
            continue;
        }
        let module_name = trimmed
            .trim_start_matches("pub mod ")
            .trim_end_matches(';')
            .trim_end_matches('{')
            .trim();
        if module_name == "api" {
            continue;
        }

        let previous_non_empty = lines[..index]
            .iter()
            .rev()
            .find(|candidate| !candidate.trim().is_empty())
            .map(|candidate| candidate.trim());
        if previous_non_empty != Some("#[doc(hidden)]") {
            violations.push(format!("{}: {}", index + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "root public modules should be documented as migration-private via #[doc(hidden)] while pi_coding_agent::api remains the stable facade:\n{}",
        violations.join("\n")
    );
}

#[test]
fn root_reexports_are_explicit_compatibility_surface() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let before_api = lib_source
        .split("pub mod api {")
        .next()
        .expect("api module should exist");
    let before_api_lines = before_api.lines().collect::<Vec<_>>();

    let mut violations = Vec::new();
    for (index, line) in before_api_lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub use ") {
            continue;
        }
        let previous_non_empty = before_api_lines[..index]
            .iter()
            .rev()
            .find(|candidate| !candidate.trim().is_empty())
            .map(|candidate| candidate.trim());
        if previous_non_empty != Some("#[deprecated(note = \"use pi_coding_agent::api instead\")]")
        {
            violations.push(format!("{}: {}", index + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "root reexports should be explicitly deprecated compatibility surface; stable users should import pi_coding_agent::api:\n{}",
        violations.join("\n")
    );
}
