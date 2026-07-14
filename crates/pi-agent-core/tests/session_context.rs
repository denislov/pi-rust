use pi_agent_core::session_context::{InMemorySessionStorage, build_session_context};
use pi_agent_core::transcript::{SessionEntry, StoredAgentMessage};
use pi_ai::types::ContentBlock;

fn user(text: &str, id: &str, parent: Option<&str>) -> SessionEntry {
    SessionEntry::message(
        id.into(),
        parent.map(str::to_string),
        "2026-06-05T00:00:00.000Z".into(),
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
fn builds_context_from_latest_linear_leaf() {
    let entries = vec![user("one", "a", None), user("two", "b", Some("a"))];
    let context = build_session_context(&entries, None).unwrap();
    assert_eq!(context.messages.len(), 2);
}

#[test]
fn builds_context_from_explicit_leaf_entry_target() {
    let mut leaf = SessionEntry {
        entry_type: "leaf".into(),
        id: "leaf0001".into(),
        parent_id: Some("b".into()),
        timestamp: "2026-06-05T00:00:01.000Z".into(),
        fields: serde_json::Map::new(),
    };
    leaf.fields
        .insert("targetId".into(), serde_json::Value::String("a".into()));
    let entries = vec![user("one", "a", None), user("two", "b", Some("a")), leaf];
    let context = build_session_context(&entries, None).unwrap();
    assert_eq!(context.messages.len(), 1);
}

#[test]
fn in_memory_storage_appends_and_tracks_leaf() {
    let mut storage = InMemorySessionStorage::new("session-1", "2026-06-05T00:00:00.000Z");
    storage.append_entry(user("one", "a", None)).unwrap();
    storage.append_entry(user("two", "b", Some("a"))).unwrap();
    assert_eq!(storage.get_leaf_id().unwrap().as_deref(), Some("b"));
    assert_eq!(storage.get_entries().len(), 2);
}
