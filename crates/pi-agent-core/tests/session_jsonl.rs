use pi_agent_core::session::{JsonlSessionStorage, SessionEntry, StoredAgentMessage};
use pi_ai::types::ContentBlock;
use serde_json::Map;

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

fn label_entry(id: &str, parent: Option<&str>, target_id: &str, label: &str) -> SessionEntry {
    let mut fields = Map::new();
    fields.insert(
        "targetId".into(),
        serde_json::Value::String(target_id.to_string()),
    );
    fields.insert("label".into(), serde_json::Value::String(label.to_string()));
    SessionEntry {
        entry_type: "label".into(),
        id: id.into(),
        parent_id: parent.map(str::to_string),
        timestamp: "2026-06-05T00:00:02.000Z".into(),
        fields,
    }
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

#[test]
fn get_entry_finds_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    assert!(storage.get_entry("u1").is_some());
    assert!(storage.get_entry("nonexistent").is_none());
}

#[test]
fn get_entry_returns_none_for_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    assert!(storage.get_entry("nonexistent").is_none());
}

#[test]
fn get_tree_linear_session() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "first"))
        .unwrap();
    storage
        .append_entry(user_entry("u2", Some("u1"), "second"))
        .unwrap();
    storage
        .append_entry(user_entry("u3", Some("u2"), "third"))
        .unwrap();

    let tree = storage.get_tree();
    assert_eq!(tree.len(), 1, "expected one root");
    assert_eq!(tree[0].entry.id, "u1", "root id");
    assert_eq!(tree[0].children.len(), 1, "u1 children count");
    assert_eq!(tree[0].children[0].entry.id, "u2", "first child");
    assert_eq!(tree[0].children[0].children.len(), 1, "u2 children count");
    assert_eq!(tree[0].children[0].children[0].entry.id, "u3", "grandchild");
    assert_eq!(
        tree[0].children[0].children[0].children.len(),
        0,
        "u3 children count"
    );
}

#[test]
fn get_tree_branching() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_entry(user_entry("u2", Some("u1"), "branch a"))
        .unwrap();
    storage
        .append_entry(user_entry("u3", Some("u1"), "branch b"))
        .unwrap();

    let tree = storage.get_tree();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].children.len(), 2);
    assert_eq!(tree[0].children[0].entry.id, "u2");
    assert_eq!(tree[0].children[1].entry.id, "u3");
}

#[test]
fn get_tree_orphan_entry_treated_as_root() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "root"))
        .unwrap();
    storage
        .append_entry(user_entry("orphan", Some("missing"), "orphan"))
        .unwrap();

    let tree = storage.get_tree();
    assert_eq!(tree.len(), 2, "orphan becomes separate root");
}

#[test]
fn get_tree_skips_leaf_entries() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage.append_leaf_marker(Some("u1")).unwrap();

    let tree = storage.get_tree();
    assert_eq!(tree.len(), 1, "leaf marker not included in tree");
    assert_eq!(tree[0].entry.id, "u1");
}

#[test]
fn get_tree_resolves_label_to_target_node() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_entry(label_entry("l1", Some("u1"), "u1", "my-label"))
        .unwrap();

    let tree = storage.get_tree();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].label.as_deref(), Some("my-label"));
}

#[test]
fn get_tree_label_does_not_appear_as_separate_node() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_entry(label_entry("l1", Some("u1"), "u1", "my-label"))
        .unwrap();

    let tree = storage.get_tree();
    assert_eq!(
        tree.len(),
        1,
        "label entry should not create a separate node"
    );
}

#[test]
fn branch_persists_leaf_marker_and_reopens_at_same_position() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_entry(user_entry("u2", Some("u1"), "world"))
        .unwrap();
    storage.branch("u1").unwrap();

    assert_eq!(
        storage.get_leaf_id().unwrap().as_deref(),
        Some("u1"),
        "leaf should be u1 after branch"
    );

    let reopened = JsonlSessionStorage::open(&file).unwrap();
    assert_eq!(
        reopened.get_leaf_id().unwrap().as_deref(),
        Some("u1"),
        "leaf persists across reopen"
    );
}

#[test]
fn branch_to_nonexistent_entry_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    let result = storage.branch("nonexistent");
    assert!(result.is_err());
}

#[test]
fn reset_leaf_appends_null_target() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage.reset_leaf().unwrap();

    let reopened = JsonlSessionStorage::open(&file).unwrap();
    assert_eq!(
        reopened.get_leaf_id().unwrap(),
        None,
        "reset leaf should set targetId to null"
    );
}

#[test]
fn append_label_change_writes_entry_and_returns_id() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();

    let label_id = storage
        .append_label_change("u1", Some("important"))
        .unwrap();
    assert!(!label_id.is_empty(), "label entry id should not be empty");

    // Verify the label is resolved in get_tree.
    let tree = storage.get_tree();
    assert_eq!(tree[0].label.as_deref(), Some("important"));
}

#[test]
fn append_label_change_with_empty_label_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_label_change("u1", Some("important"))
        .unwrap();
    storage.append_label_change("u1", None).unwrap();

    let tree = storage.get_tree();
    assert_eq!(
        tree[0].label.as_deref(),
        Some(""),
        "label resolved as empty string"
    );
}

#[test]
fn append_label_change_does_not_change_leaf() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_label_change("u1", Some("important"))
        .unwrap();
    assert_eq!(
        storage.get_leaf_id().unwrap().as_deref(),
        Some("u1"),
        "leaf should remain u1 after label change"
    );
}

#[test]
fn get_tree_children_sorted_by_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "root"))
        .unwrap();
    // Add children with different timestamps
    let mut entry_b = user_entry("u2", Some("u1"), "second");
    entry_b.timestamp = "2026-06-05T00:00:03.000Z".into();
    storage.append_entry(entry_b).unwrap();
    let mut entry_a = user_entry("u3", Some("u1"), "first");
    entry_a.timestamp = "2026-06-05T00:00:02.000Z".into();
    storage.append_entry(entry_a).unwrap();

    let tree = storage.get_tree();
    assert_eq!(tree[0].children.len(), 2);
    assert_eq!(
        tree[0].children[0].entry.id, "u3",
        "earlier timestamp first"
    );
    assert_eq!(tree[0].children[1].entry.id, "u2", "later timestamp second");
}

#[test]
fn leaf_marker_reopen_restores_leaf_id() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    let mut storage =
        JsonlSessionStorage::create(&file, "/tmp", "s1", "2026-06-05T00:00:00.000Z", None).unwrap();
    storage
        .append_entry(user_entry("u1", None, "hello"))
        .unwrap();
    storage
        .append_entry(user_entry("u2", Some("u1"), "world"))
        .unwrap();
    storage.append_leaf_marker(Some("u1")).unwrap();

    let reopened = JsonlSessionStorage::open(&file).unwrap();
    assert_eq!(
        reopened.get_leaf_id().unwrap().as_deref(),
        Some("u1"),
        "leaf_id restored to u1 after reopen"
    );
}
