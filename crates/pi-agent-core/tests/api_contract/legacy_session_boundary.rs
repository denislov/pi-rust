//! Removal contract for the retired core session facade.

#[test]
fn legacy_session_facade_is_removed_from_pi_agent_core() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = std::fs::read_to_string(crate_dir.join("src/lib.rs"))
        .expect("crate lib should be readable");

    assert!(
        !crate_dir.join("src/session").exists(),
        "legacy pi_agent_core::session facade source directory should be removed"
    );
    assert!(
        !lib_rs.contains("pub mod session;"),
        "pi-agent-core should not export the legacy pi_agent_core::session module"
    );
    assert!(
        !lib_rs.contains("pi_agent_core::session for"),
        "replacement guidance should live on current modules, not preserve a legacy session facade"
    );
}

#[test]
fn legacy_jsonl_storage_and_repo_are_removed_from_pi_agent_core() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative in [
        "src/session/jsonl.rs",
        "src/session/repo.rs",
        "src/session/migrations.rs",
    ] {
        assert!(
            !crate_dir.join(relative).exists(),
            "legacy JSONL session storage/repo source should be removed: {relative}"
        );
    }

    let transcript_types = std::fs::read_to_string(crate_dir.join("src/transcript/types.rs"))
        .expect("transcript types should be readable");
    assert!(
        !transcript_types.contains("JsonlSessionMetadata"),
        "legacy JSONL metadata type should stay out of shared transcript types"
    );
}

#[test]
fn session_context_sources_live_outside_legacy_session_module() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative in [
        "src/session/context.rs",
        "src/session/error.rs",
        "src/session/memory.rs",
    ] {
        assert!(
            !crate_dir.join(relative).exists(),
            "real session-context source should not live under legacy session path: {relative}"
        );
    }

    let session_context_dir = crate_dir.join("src/context");
    for relative in ["assembly.rs", "error.rs", "memory.rs"] {
        assert!(
            session_context_dir.join(relative).is_file(),
            "session-context source should live under src/context/{relative}"
        );
    }
}
