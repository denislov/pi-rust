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

#[test]
fn coding_agent_session_owner_does_not_define_delegation_runtime_seed_mapping() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let delegation_source = fs::read_to_string(crate_root.join("src/coding_session/delegation.rs"))
        .expect("delegation source should be readable");

    assert!(
        !owner_source.contains("fn pending_state_from_replay")
            && !owner_source.contains("fn prompt_options_from_delegation_runtime_seed")
            && !owner_source.contains("fn delegation_runtime_seed_from_prompt_options"),
        "CodingAgentSession owner should not define delegation runtime seed mapping helpers"
    );
    assert!(
        delegation_source.contains("fn pending_state_from_replay")
            && delegation_source.contains("fn prompt_options_from_delegation_runtime_seed")
            && delegation_source.contains("fn delegation_runtime_seed_from_prompt_options"),
        "delegation boundary should own delegation runtime seed mapping helpers"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_delegation_confirmation_service_glue() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let service_source = fs::read_to_string(
        crate_root.join("src/coding_session/delegation_confirmation_service.rs"),
    )
    .unwrap_or_default();

    assert!(
        !owner_source.contains("fn pending_delegation_confirmation_not_found")
            && !owner_source.contains("fn adopt_pending_delegation_confirmations")
            && !owner_source.contains("fn queue_pending_delegation_confirmation")
            && !owner_source.contains("fn record_delegation_confirmation_requested")
            && !owner_source.contains("fn record_delegation_confirmation_approved")
            && !owner_source.contains("fn record_delegation_confirmation_rejected"),
        "CodingAgentSession owner should not define delegation confirmation queue/persistence glue"
    );
    assert!(
        service_source.contains("struct DelegationConfirmationService")
            && service_source.contains("fn adopt_pending")
            && service_source.contains("fn queue_pending")
            && service_source.contains("fn approve_pending")
            && service_source.contains("fn reject_pending"),
        "DelegationConfirmationService should own pending delegation confirmation queue and persistence glue"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_approved_delegation_execution() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let delegation_execution_source =
        fs::read_to_string(crate_root.join("src/coding_session/delegation_execution_service.rs"))
            .unwrap_or_default();

    assert!(
        !owner_source.contains("struct ApprovedDelegationExecution")
            && !owner_source.contains("async fn execute_approved_agent_delegation")
            && !owner_source.contains("async fn execute_approved_team_delegation"),
        "CodingAgentSession owner should not define approved delegation execution helpers"
    );
    assert!(
        delegation_execution_source.contains("struct DelegationExecutionService")
            && delegation_execution_source.contains("struct ApprovedDelegationExecution")
            && delegation_execution_source.contains("AgentInvocationContext::new(")
            && delegation_execution_source.contains("AgentTeamContext::new("),
        "DelegationExecutionService should own approved delegated flow context construction"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_plugin_load_execution() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let service_source =
        fs::read_to_string(crate_root.join("src/coding_session/plugin_load_service.rs"))
            .unwrap_or_default();

    assert!(
        !owner_source.contains("PluginLoadContext::new(")
            && !owner_source.contains("run_plugin_load(")
            && !owner_source.contains("begin_plugin_load_transaction")
            && !owner_source.contains("record_plugin_load_completed")
            && !owner_source.contains("commit_plugin_load_transaction")
            && !owner_source.contains("fail_plugin_load_transaction"),
        "CodingAgentSession owner should not define plugin load execution plumbing"
    );
    assert!(
        service_source.contains("struct PluginLoadService")
            && service_source.contains("PluginLoadContext::new(")
            && service_source.contains("run_plugin_load")
            && service_source.contains("commit_plugin_load_transaction")
            && service_source.contains("fail_plugin_load_transaction"),
        "PluginLoadService should own plugin load flow execution plumbing"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_branch_summary_execution() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let service_source =
        fs::read_to_string(crate_root.join("src/coding_session/branch_summary_service.rs"))
            .unwrap_or_default();

    assert!(
        !owner_source.contains("fn reused_branch_summary_outcome")
            && !owner_source.contains("async fn summarize_branch_inner")
            && !owner_source.contains("BranchSummaryContext::new("),
        "CodingAgentSession owner should not define branch summary execution plumbing"
    );
    assert!(
        service_source.contains("struct BranchSummaryService")
            && service_source.contains("BranchSummaryContext::new(")
            && service_source.contains("run_branch_summary"),
        "BranchSummaryService should own branch summary flow execution plumbing"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_manual_compaction_execution() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let service_source =
        fs::read_to_string(crate_root.join("src/coding_session/manual_compaction_service.rs"))
            .unwrap_or_default();

    assert!(
        !owner_source.contains("async fn compact_inner")
            && !owner_source.contains("ManualCompactionContext::new(")
            && !owner_source.contains("run_manual_compaction"),
        "CodingAgentSession owner should not define manual compaction execution plumbing"
    );
    assert!(
        service_source.contains("struct ManualCompactionService")
            && service_source.contains("ManualCompactionContext::new(")
            && service_source.contains("run_manual_compaction"),
        "ManualCompactionService should own manual compaction flow execution plumbing"
    );
}

#[test]
fn coding_agent_session_owner_does_not_define_self_healing_edit_execution() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let owner_source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner source should be readable");
    let service_source =
        fs::read_to_string(crate_root.join("src/coding_session/self_healing_edit_service.rs"))
            .unwrap_or_default();

    assert!(
        !owner_source.contains("async fn self_healing_edit_inner")
            && !owner_source.contains("SelfHealingEditContext::new("),
        "CodingAgentSession owner should not define self-healing edit execution plumbing"
    );
    assert!(
        service_source.contains("struct SelfHealingEditService")
            && service_source.contains("SelfHealingEditContext::new(")
            && service_source.contains("run_self_healing_edit"),
        "SelfHealingEditService should own self-healing edit flow execution plumbing"
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
