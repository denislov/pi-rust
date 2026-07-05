use std::fs;
use std::path::{Path, PathBuf};

const TEST_FILES_ALLOWED_TO_IMPORT_CORE_SESSION: &[&str] =
    &["crates/pi-coding-agent/tests/session_boundary_guards.rs"];

#[test]
fn production_code_uses_transcript_boundary_for_core_session_types() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-coding-agent");
    let src_root = crate_root.join("src");
    let mut violations = Vec::new();

    collect_core_session_imports(repo_root, &src_root, &mut violations);

    assert!(
        violations.is_empty(),
        "pi-coding-agent production code should import shared transcript/tree/id types through pi_agent_core::transcript, not pi_agent_core::session:\n{}",
        violations.join("\n")
    );
}

#[test]
fn transcript_only_tests_use_transcript_boundary_for_core_session_types() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-coding-agent");
    let tests_root = crate_root.join("tests");
    let mut violations = Vec::new();

    collect_core_session_imports_with_allowlist(
        repo_root,
        &tests_root,
        TEST_FILES_ALLOWED_TO_IMPORT_CORE_SESSION,
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "pi-coding-agent tests should import shared transcript/tree/id types through pi_agent_core::transcript, not pi_agent_core::session:\n{}",
        violations.join("\n")
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_self_healing_edit_event_observer() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let event_service_source =
        fs::read_to_string(crate_root.join("src/coding_session/event_service.rs"))
            .expect("event service source should be readable");

    assert!(
        !owner_source.contains("struct SelfHealingEditEventObserver")
            && !owner_source
                .contains("impl SelfHealingEditObserver for SelfHealingEditEventObserver"),
        "CodingAgentSession owner should not define the self-healing edit event observer; live repair-attempt event emission belongs behind EventService"
    );
    assert!(
        event_service_source.contains("struct SelfHealingEditEventObserver")
            && event_service_source
                .contains("impl SelfHealingEditObserver for SelfHealingEditEventObserver"),
        "EventService should own the self-healing edit event observer"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_session_persistence_state() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let session_service_source =
        fs::read_to_string(crate_root.join("src/coding_session/session_service.rs"))
            .expect("session service source should be readable");

    assert!(
        !owner_source.contains("enum SessionPersistence")
            && !owner_source.contains("struct TransientSessionState"),
        "CodingAgentSession owner should not define session persistence state; persistent/transient state belongs behind SessionService"
    );
    assert!(
        session_service_source.contains("enum SessionPersistence")
            && session_service_source.contains("struct TransientSessionState"),
        "SessionService should own session persistence state"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_pending_delegation_queue_state() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let delegation_source = fs::read_to_string(crate_root.join("src/coding_session/delegation.rs"))
        .expect("delegation source should be readable");

    assert!(
        !owner_source.contains("struct PendingDelegationConfirmationState")
            && !owner_source.contains("fn delegation_confirmation_is_expired")
            && !owner_source.contains("fn pending_delegation_confirmation_index"),
        "CodingAgentSession owner should not define pending delegation queue state, TTL checks, or lookup helpers"
    );
    assert!(
        delegation_source.contains("struct PendingDelegationConfirmationQueue")
            && delegation_source.contains("struct PendingDelegationConfirmationState")
            && delegation_source.contains("fn delegation_confirmation_is_expired"),
        "delegation boundary should own pending delegation queue state and TTL checks"
    );
}

fn collect_core_session_imports(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    collect_core_session_imports_with_allowlist(repo_root, path, &[], violations);
}

fn collect_core_session_imports_with_allowlist(
    repo_root: &Path,
    path: &Path,
    allowed_files: &[&str],
    violations: &mut Vec<String>,
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
            collect_core_session_imports_with_allowlist(
                repo_root,
                &entry.path(),
                allowed_files,
                violations,
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
    let mut in_pi_agent_core_grouped_use = false;
    for (line_index, line) in content.lines().enumerate() {
        if line.contains("pi_agent_core::session") {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }

        if line.contains("use pi_agent_core::{") {
            in_pi_agent_core_grouped_use = true;
        }
        if in_pi_agent_core_grouped_use && line.contains("session::") {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
        if in_pi_agent_core_grouped_use && line.contains("};") {
            in_pi_agent_core_grouped_use = false;
        }
    }
}
