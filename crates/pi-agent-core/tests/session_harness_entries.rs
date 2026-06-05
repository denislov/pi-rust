use pi_agent_core::ThinkingLevel;
use pi_agent_core::session::types::SessionEntry;
use serde_json::Value;

#[test]
fn compaction_entry_serializes_as_typescript_shape() {
    let entry = SessionEntry::compaction(
        "cmp00001".into(),
        Some("msg00001".into()),
        "2026-06-05T00:00:00.000Z".into(),
        "summary".into(),
        "msg00010".into(),
        12345,
        Some(serde_json::json!({
            "readFiles": ["README.md"],
            "modifiedFiles": ["src/lib.rs"]
        })),
        false,
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "compaction");
    assert_eq!(json["summary"], "summary");
    assert_eq!(json["firstKeptEntryId"], "msg00010");
    assert_eq!(json["tokensBefore"], 12345);
    assert_eq!(json["fromHook"], false);
}

#[test]
fn thinking_level_change_serializes_as_typescript_shape() {
    let entry = SessionEntry::thinking_level_change(
        "think001".into(),
        None,
        "2026-06-05T00:00:00.000Z".into(),
        ThinkingLevel::High,
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "thinking_level_change");
    assert_eq!(json["thinkingLevel"], "high");
}

#[test]
fn model_change_entry() {
    let entry = SessionEntry::model_change(
        "mc001".into(),
        Some("parent".into()),
        "2026-06-05T00:00:00.000Z".into(),
        "model-a".into(),
        "model-b".into(),
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "model_change");
    assert_eq!(json["from"], "model-a");
    assert_eq!(json["to"], "model-b");
}

#[test]
fn active_tools_change_entry() {
    let entry = SessionEntry::active_tools_change(
        "at001".into(),
        None,
        "2026-06-05T00:00:00.000Z".into(),
        vec!["read".into(), "write".into()],
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "active_tools_change");
    let tools = json["activeTools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0], "read");
    assert_eq!(tools[1], "write");
}
