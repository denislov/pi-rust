use pi_agent_core::session::{JsonlSessionStorage, SessionEntry, StoredAgentMessage};
use pi_ai::types::ContentBlock;

fn user_entry(id: &str, parent: Option<&str>, text: &str) -> SessionEntry {
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
fn creates_header_and_appends_entries() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage = JsonlSessionStorage::create(
        &file,
        "/tmp/project",
        "session-1",
        "2026-06-05T00:00:00.000Z",
        None,
    )
    .unwrap();
    storage
        .append_entry(user_entry("entry001", None, "hello"))
        .unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains(r#""type":"session""#));
    assert!(lines[1].contains(r#""role":"user""#));
}

#[test]
fn opens_existing_file_and_tracks_latest_leaf() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    std::fs::write(
        &file,
        concat!(
            r#"{"type":"session","version":3,"id":"session-1","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp/project"}"#,
            "\n",
            r#"{"type":"message","id":"entry001","parentId":null,"timestamp":"2026-06-05T00:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"hello"}],"timestamp":1}}"#,
            "\n"
        ),
    )
    .unwrap();
    let storage = JsonlSessionStorage::open(&file).unwrap();
    assert_eq!(storage.header().id, "session-1");
    assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("entry001"));
    assert_eq!(storage.get_entries().len(), 1);
}

#[test]
fn rejects_missing_header() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.jsonl");
    std::fs::write(
        &file,
        r#"{"type":"message","id":"x","parentId":null,"timestamp":"now"}"#,
    )
    .unwrap();
    let error = JsonlSessionStorage::open(&file).unwrap_err();
    assert!(
        error
            .message
            .contains("first line is not a valid session header")
    );
}
