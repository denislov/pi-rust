use pi_agent_core::session::{
    SessionEntry, SessionHeader, StoredAgentMessage, StoredUsage, StoredUsageCost,
};
use pi_ai::types::{ContentBlock, StopReason};

#[test]
fn header_serializes_as_jsonl_v3_header() {
    let header = SessionHeader {
        entry_type: "session".into(),
        version: 3,
        id: "019de8c2-de29-73e9-ae0c-e134db34c447".into(),
        timestamp: "2026-06-05T00:00:00.000Z".into(),
        cwd: "/tmp/project".into(),
        parent_session: Some("/tmp/source.jsonl".into()),
    };
    let json = serde_json::to_string(&header).unwrap();
    assert_eq!(
        json,
        r#"{"type":"session","version":3,"id":"019de8c2-de29-73e9-ae0c-e134db34c447","timestamp":"2026-06-05T00:00:00.000Z","cwd":"/tmp/project","parentSession":"/tmp/source.jsonl"}"#
    );
}

#[test]
fn user_message_entry_matches_typescript_shape() {
    let entry = SessionEntry::message(
        "entry001".into(),
        None,
        "2026-06-05T00:00:01.000Z".into(),
        StoredAgentMessage::User {
            content: vec![ContentBlock::Text {
                text: "hello".into(),
                text_signature: None,
            }],
            timestamp: 1_780_588_800_000,
        },
    );
    let value = serde_json::to_value(&entry).unwrap();
    assert_eq!(value["type"], "message");
    assert_eq!(value["parentId"], serde_json::Value::Null);
    assert_eq!(value["message"]["role"], "user");
    assert_eq!(value["message"]["content"][0]["type"], "text");
}

#[test]
fn assistant_usage_uses_typescript_total_field() {
    let entry = SessionEntry::message(
        "entry002".into(),
        Some("entry001".into()),
        "2026-06-05T00:00:02.000Z".into(),
        StoredAgentMessage::Assistant {
            content: vec![ContentBlock::Text {
                text: "hi".into(),
                text_signature: None,
            }],
            api: "anthropic-messages".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            response_model: None,
            response_id: None,
            usage: StoredUsage {
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                total: 0,
                cost: StoredUsageCost::default(),
            },
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 1_780_588_801_000,
        },
    );
    let value = serde_json::to_value(&entry).unwrap();
    assert_eq!(value["message"]["role"], "assistant");
    assert!(value["message"]["usage"].get("total").is_some());
    assert!(value["message"]["usage"].get("totalTokens").is_none());
    assert_eq!(value["message"]["stopReason"], "stop");
}

#[test]
fn leaf_entry_roundtrips_without_losing_target_id() {
    let raw = r#"{"type":"leaf","id":"leaf0001","parentId":"entry002","timestamp":"2026-06-05T00:00:03.000Z","targetId":"entry001"}"#;
    let entry: SessionEntry = serde_json::from_str(raw).unwrap();
    assert_eq!(entry.entry_type, "leaf");
    assert_eq!(
        entry.field("targetId").and_then(|v| v.as_str()),
        Some("entry001")
    );
    assert_eq!(serde_json::to_string(&entry).unwrap(), raw);
}
