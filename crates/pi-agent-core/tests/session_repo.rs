use pi_agent_core::session::{JsonlSessionRepo, SessionEntry, StoredAgentMessage};
use pi_ai::types::ContentBlock;

fn user(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        "2026-06-05T00:00:01.000Z".into(),
        StoredAgentMessage::User {
            content: vec![ContentBlock::Text {
                text: text.into(),
                text_signature: None,
            }],
            timestamp: 1,
        },
    )
}

#[test]
fn encodes_cwd_like_typescript() {
    assert_eq!(
        JsonlSessionRepo::encode_cwd("/home/me/project"),
        "--home-me-project--"
    );
}

#[test]
fn encodes_cwd_windows_path() {
    // TS: cwd.replace(/^[/\\]/, "").replace(/[/\\:]/g, "-")
    assert_eq!(
        JsonlSessionRepo::encode_cwd("D:\\Workspace\\pi2rust\\pi-rust"),
        "--D--Workspace-pi2rust-pi-rust--"
    );
}

#[test]
fn encodes_cwd_mixed_slashes_and_colon() {
    assert_eq!(
        JsonlSessionRepo::encode_cwd("C:/Users/name/project"),
        "--C--Users-name-project--"
    );
}

#[test]
fn creates_lists_and_opens_by_id_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JsonlSessionRepo::new(dir.path());
    let mut session = repo
        .create("/tmp/project", Some("019de8c2-de29-73e9-ae0c-e134db34c447"))
        .unwrap();
    session
        .append_entry(user("entry001", None, "hello"))
        .unwrap();
    let listed = repo.list(Some("/tmp/project")).unwrap();
    assert_eq!(listed.len(), 1);
    let opened = repo.open_target("/tmp/project", "019de8c2").unwrap();
    assert_eq!(opened.header().id, "019de8c2-de29-73e9-ae0c-e134db34c447");
}

#[test]
fn forks_session_with_parent_session_header() {
    let dir = tempfile::tempdir().unwrap();
    let repo = JsonlSessionRepo::new(dir.path());
    let mut source = repo.create("/tmp/project", Some("source-session")).unwrap();
    source
        .append_entry(user("entry001", None, "hello"))
        .unwrap();
    let fork = repo
        .fork(source.path(), "/tmp/project", Some("fork-session"), None)
        .unwrap();
    assert_eq!(
        fork.header().parent_session.as_deref(),
        Some(source.path().to_str().unwrap())
    );
    assert_eq!(fork.get_entries().len(), 1);
}
