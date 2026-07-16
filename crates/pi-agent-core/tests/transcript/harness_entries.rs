//! Transcript entry serialization used by the agent harness.

use pi_agent_core::api::agent::ThinkingLevel;
use pi_agent_core::api::transcript::SessionEntry;

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
        "anthropic".into(),
        "claude-sonnet-4-5".into(),
    );
    let json = serde_json::to_value(entry).unwrap();
    assert_eq!(json["type"], "model_change");
    assert_eq!(json["provider"], "anthropic");
    assert_eq!(json["modelId"], "claude-sonnet-4-5");
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
    let tools = json["activeToolNames"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0], "read");
    assert_eq!(tools[1], "write");
}
