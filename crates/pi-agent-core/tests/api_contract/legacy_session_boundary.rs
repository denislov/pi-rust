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
fn session_context_subsystem_is_removed_from_pi_agent_core() {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    for relative in [
        "src/session/context.rs",
        "src/session/error.rs",
        "src/session/memory.rs",
        "src/context/assembly.rs",
        "src/context/error.rs",
        "src/context/memory.rs",
    ] {
        assert!(
            !crate_dir.join(relative).exists(),
            "test-only session-context subsystem should be removed: {relative}"
        );
    }

    let context_mod = std::fs::read_to_string(crate_dir.join("src/context/mod.rs"))
        .expect("context module should be readable");
    assert!(
        !context_mod.contains("pub mod assembly"),
        "context module should not export the retired assembly submodule"
    );
    assert!(
        !context_mod.contains("pub mod memory"),
        "context module should not export the retired memory submodule"
    );
    assert!(
        !context_mod.contains("pub mod error"),
        "context module should not export the retired session error submodule"
    );
    assert!(
        !context_mod.contains("SessionContext"),
        "context module should not re-export the retired SessionContext type"
    );
    assert!(
        !context_mod.contains("InMemorySessionStorage"),
        "context module should not re-export the retired InMemorySessionStorage type"
    );

    let api_rs = std::fs::read_to_string(crate_dir.join("src/api.rs"))
        .expect("api facade should be readable");
    assert!(
        !api_rs.contains("SessionContext"),
        "api facade should not re-export the retired SessionContext type"
    );
    assert!(
        !api_rs.contains("InMemorySessionStorage"),
        "api facade should not re-export the retired InMemorySessionStorage type"
    );
    assert!(
        !api_rs.contains("build_session_context"),
        "api facade should not re-export the retired build_session_context helper"
    );
}
